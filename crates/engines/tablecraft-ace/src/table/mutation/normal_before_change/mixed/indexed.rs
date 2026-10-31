//! Complete ordinary scalar tables with native-derived indexed I32 targets.
#[path = "indexed/selection.rs"]
mod selection;
use super::super::super::{
    EditError, Int32Update, NativeEditPlan, WriteBlocker,
    guards::{buffer, fields::batch, native_events},
    lval_ownership::Inventory,
    preserved_source::{self, Source, validation},
};
use super::super::event_error;
use super::{prepare_with_indexes, proof, stage, validate_with_indexes};
use crate::{
    IndexCheckStatus, InspectError, NativeImage, TableSchema, check_table,
    format::{
        layout::CALCULATED_EXT_MASK,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    preservation::COPY_CHUNK_BYTES,
    table::{
        indexes::{
            IndexKeyUpdate, MutationKey,
            mutation::{normal_fitting, normal_index_audit},
        },
        record_layout,
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

struct Admission<'image> {
    source: Source,
    targets: batch::CheckedTargets<'image>,
}

pub(super) fn selected(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    if schema.layout.physical_indexes == 0 {
        return Ok(false);
    }
    for field in schema.columns() {
        check(budget, cancelled)?;
        if field.scalar_type().is_none() || field.extended_flags() & CALCULATED_EXT_MASK != 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn capture<'image>(
    image: &'image NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Admission<'image>, EditError> {
    let targets = batch::CheckedTargets::capture_native(image, table, budget, cancelled)?;
    let checked = targets.checked();
    let programs =
        native_events::programs(image, checked.schema(), budget, cancelled).map_err(event_error)?;
    let source = preserved_source::capture_checked_before_change(
        image, table, &programs, checked, budget, cancelled,
    )?;
    let ownership = normal_index_audit::Ownership::capture(image, checked, budget, cancelled)?;
    let live = Inventory::capture(image, budget, cancelled)?;
    for index in checked.indexes() {
        check(budget, cancelled)?;
        let tree = ownership.require(image, checked.schema(), index.index(), budget, cancelled)?;
        live.require_no_long_value_data_overlap(tree.pages(), budget, cancelled)?;
    }
    // Full reader coverage does not make nullable keys writable by the current
    // manual I32 tree codec. Refuse this shape before executing event rows.
    for record in &source.records {
        let layout = record_layout::parse(&record.data, source.table.schema(), budget)?;
        for index in checked.indexes() {
            for key in index.index().columns() {
                check(budget, cancelled)?;
                budget
                    .charge(
                        Resource::Steps,
                        source.table.schema().columns().len() as u64,
                    )
                    .map_err(InspectError::from)?;
                let field = source
                    .table
                    .schema()
                    .columns()
                    .iter()
                    .find(|field| field.number() == key.number())
                    .ok_or(EditError::Preservation)?;
                let end = COLUMN_COUNT_BYTES
                    .checked_add(field.field.offset)
                    .and_then(|start| start.checked_add(size_of::<i32>()))
                    .ok_or(EditError::Preservation)?;
                let mask = layout
                    .mask_start
                    .checked_add(field.number() / MASK_BITS)
                    .ok_or(EditError::Preservation)?;
                if end > layout.mask_start
                    || record
                        .data
                        .get(mask)
                        .is_none_or(|byte| byte & (1 << (field.number() % MASK_BITS)) == 0)
                {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            }
        }
    }
    Ok(Admission { source, targets })
}

pub(super) fn preflight(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if matches!(
        selection::capture(image, table, budget, cancelled)?,
        selection::Route::LegacyUnindexed
    ) {
        return super::preflight_unindexed(image, table, budget, cancelled);
    }
    let admission = capture(image, table, budget, cancelled)?;
    for (position, field) in admission.source.table.schema().columns().iter().enumerate() {
        check(budget, cancelled)?;
        match stage::target_with_indexes(
            &admission.source,
            position,
            Some(&admission.targets),
            budget,
            cancelled,
        ) {
            Ok(()) => {
                prepare_with_indexes(
                    image,
                    &admission.source,
                    field.name(),
                    &[],
                    Some(&admission.targets),
                    budget,
                    cancelled,
                )?;
                return Ok(());
            }
            Err(EditError::Unsupported(WriteBlocker::Column | WriteBlocker::IndexedColumn)) => {}
            Err(error) => return Err(error),
        }
    }
    Err(EditError::Unsupported(WriteBlocker::Column))
}

fn index_updates(
    source: &Source,
    prepared: &batch::Prepared<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexKeyUpdate>, EditError> {
    let groups = prepared.final_groups().ok_or(EditError::Preservation)?;
    let capacity = groups.iter().try_fold(0_usize, |total, group| {
        total
            .checked_add(group.updates().len())
            .ok_or(InspectError::StructureLimit)
    })?;
    let mut keys = buffer::vector(capacity, budget, cancelled)?;
    for group in groups {
        let field = source
            .table
            .schema()
            .columns()
            .get(group.column())
            .ok_or(EditError::Preservation)?;
        let mut indexed = false;
        for index in source.indexes.indexes() {
            for key in index.columns() {
                check(budget, cancelled)?;
                indexed |= key.number() == field.number();
            }
        }
        if !indexed {
            continue;
        }
        for update in group.updates() {
            check(budget, cancelled)?;
            let (Value::Int32(before), Value::Int32(after)) = (update.expected(), update.value())
            else {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            };
            if before == after {
                continue;
            }
            let record = source
                .records
                .get(update.row())
                .ok_or(EditError::Conflict)?;
            let kind = MutationKey::new(ScalarType::Int32)?;
            keys.push(IndexKeyUpdate {
                reference: record.reference,
                column: group.column(),
                expected: Some(kind.stored(&before.to_le_bytes())?),
                value: Some(kind.stored(&after.to_le_bytes())?),
            });
        }
    }
    Ok(keys)
}

fn checked_after(
    image: &NativeImage,
    table: &str,
    source: &Source,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    validation::expected(image, table, &source.table, after, budget, cancelled)?;
    let checked = check_table(
        &mut Cursor::new(after),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    if checked.schema().object_id() != source.table.schema().object_id()
        || checked.schema().definition_page() != source.table.schema().definition_page()
        || checked.rows() != source.records.len()
        || checked
            .indexes()
            .iter()
            .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Preservation);
    }
    Ok(())
}

pub(super) fn plan(
    image: NativeImage,
    table: &str,
    column: &str,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let _span = tracing::debug_span!("native.before_change.normal.indexed_mixed.plan").entered();
    if matches!(
        selection::capture(&image, table, budget, cancelled)?,
        selection::Route::LegacyUnindexed
    ) {
        return super::plan_unindexed(image, table, column, updates, budget, cancelled);
    }
    let admission = capture(&image, table, budget, cancelled)?;
    prepare_with_indexes(
        &image,
        &admission.source,
        column,
        &[],
        Some(&admission.targets),
        budget,
        cancelled,
    )?;
    validate_with_indexes(
        &admission.source,
        column,
        updates,
        Some(&admission.targets),
        budget,
        cancelled,
    )?;
    let mut prepared = prepare_with_indexes(
        &image,
        &admission.source,
        column,
        updates,
        Some(&admission.targets),
        budget,
        cancelled,
    )?;
    if let Some(rejection) = prepared.take_rejection() {
        return Err(EditError::BeforeChangeRejected(rejection));
    }
    let mut staged = stage::derive_with_indexes(
        &image,
        &admission.source,
        &prepared,
        Some(&admission.targets),
        budget,
        cancelled,
    )?;
    let keys = index_updates(&admission.source, &prepared, budget, cancelled)?;
    let patches = normal_fitting::patches(
        &image,
        admission.targets.checked(),
        &keys,
        budget,
        cancelled,
    )?;
    for patch in patches {
        check(budget, cancelled)?;
        let end = patch
            .offset
            .checked_add(patch.before.len())
            .ok_or(EditError::Preservation)?;
        let bytes = staged
            .bytes
            .get_mut(patch.offset..end)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(
                Resource::Steps,
                (patch.before.len() + patch.after.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if patch.before.len() != patch.after.len() || bytes != patch.before {
            return Err(EditError::Preservation);
        }
        bytes.copy_from_slice(&patch.after);
    }
    checked_after(
        &image,
        table,
        &admission.source,
        &staged.bytes,
        budget,
        cancelled,
    )?;
    drop(prepared);
    drop(admission);
    let scope = image.catalog().catalog().scope();
    let plan = NativeEditPlan {
        before: image.bytes,
        after: staged.bytes,
        changed_rows: staged.changed_rows,
    };
    prove(&plan, scope, table, column, updates, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    column: &str,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.before_change.normal.indexed_mixed.prove").entered();
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    if matches!(
        selection::capture(&image, table, budget, cancelled)?,
        selection::Route::LegacyUnindexed
    ) {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let admission = capture(&image, table, budget, cancelled)?;
    proof::original_frames(&admission.source, budget, cancelled)?;
    prepare_with_indexes(
        &image,
        &admission.source,
        column,
        &[],
        Some(&admission.targets),
        budget,
        cancelled,
    )?;
    validate_with_indexes(
        &admission.source,
        column,
        updates,
        Some(&admission.targets),
        budget,
        cancelled,
    )?;
    let prepared = prepare_with_indexes(
        &image,
        &admission.source,
        column,
        updates,
        Some(&admission.targets),
        budget,
        cancelled,
    )?;
    if prepared.rejection().is_some() {
        return Err(EditError::Preservation);
    }
    let (mut expected, changed) = proof::reconstruct_source(
        &image,
        &admission.source,
        &prepared,
        Some(&admission.targets),
        budget,
        cancelled,
    )?;
    let groups = prepared.final_groups().ok_or(EditError::Preservation)?;
    normal_index_audit::reconstruct_projected(
        &image,
        admission.targets.checked(),
        &mut expected,
        |ordinal, position, original, budget| {
            let mut found = None;
            for group in groups {
                check(budget, cancelled)?;
                if group.column() != position {
                    continue;
                }
                for update in group.updates() {
                    check(budget, cancelled)?;
                    if update.row() != ordinal {
                        continue;
                    }
                    let (Value::Int32(before), Value::Int32(after)) =
                        (update.expected(), update.value())
                    else {
                        return Err(EditError::Unsupported(WriteBlocker::Column));
                    };
                    if *before != original || found.replace(*after).is_some() {
                        return Err(EditError::Preservation);
                    }
                }
            }
            Ok(found.unwrap_or(original))
        },
        budget,
        cancelled,
    )?;
    checked_after(
        &image,
        table,
        &admission.source,
        &expected,
        budget,
        cancelled,
    )?;
    if expected.len() != plan.after_bytes().len() || changed != plan.changed_rows() {
        return Err(EditError::Preservation);
    }
    for (before, after) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(plan.after_bytes().chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, before.len() as u64)
            .map_err(InspectError::from)?;
        if before != after {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

#[cfg(test)]
#[path = "indexed/proof_tests.rs"]
mod proof_tests;

#[cfg(test)]
#[path = "indexed/nullable_tests.rs"]
mod nullable_tests;
