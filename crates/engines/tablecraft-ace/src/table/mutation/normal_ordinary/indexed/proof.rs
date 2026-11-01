//! Independent ordinary record and nullable-index replay from fresh BEFORE.
use crate::{
    EditError, FixedColumnUpdates, IndexCheckStatus, InspectError, NativeEditPlan, NativeImage,
    TableSchema, check_table,
    header::check,
    preservation::COPY_CHUNK_BYTES,
    table::{
        indexes::mutation::normal_index_audit,
        mutation::{
            fixed_records::{self, indexed::CheckedIndexes, proof as records},
            normal_ordinary::capture,
            preserved_source::validation,
        },
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.ordinary.indexed.prove").entered();
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = capture(&image, table, budget, cancelled)?;
    let facts = CheckedIndexes::capture(&image, table, budget, cancelled)?;
    let expected = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    let (mut expected, changed_rows) = records::reconstruct_original(
        records::Original {
            image: &image,
            source: &source,
        },
        expected,
        groups,
        Some(&facts),
        budget,
        cancelled,
    )?;
    normal_index_audit::reconstruct_nullable_projected(
        &image,
        facts.checked(),
        &mut expected,
        |ordinal, position, original, budget| {
            requested(
                source.table.schema(),
                groups,
                ordinal,
                position,
                original,
                budget,
                cancelled,
            )
        },
        budget,
        cancelled,
    )?;

    // Validate the complete result against ORIGINAL properties after records
    // and indexes have both been independently rebuilt. No staged group,
    // encoded key, program report or candidate metadata supplied authority.
    validation::expected(&image, table, &source.table, &expected, budget, cancelled)?;
    let checked = check_table(&mut Cursor::new(&expected), scope, table, budget, cancelled)?;
    if checked.schema().object_id() != source.table.schema().object_id()
        || checked.schema().definition_page() != source.table.schema().definition_page()
        || checked.rows() != source.records.len()
        || checked.indexes().len() != facts.checked().indexes().len()
        || checked
            .indexes()
            .iter()
            .any(|entry| entry.status() != IndexCheckStatus::Checked)
        || changed_rows != plan.changed_rows()
        || expected.len() != plan.after_bytes().len()
    {
        return Err(EditError::Preservation);
    }
    for (expected, observed) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(plan.after_bytes().chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != observed {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

fn requested(
    schema: &TableSchema,
    groups: &[FixedColumnUpdates<'_>],
    ordinal: usize,
    position: usize,
    original: Option<i32>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<i32>, EditError> {
    let field = schema
        .columns()
        .get(position)
        .ok_or(EditError::Preservation)?;
    let mut replacement = None;
    for group in groups {
        check(budget, cancelled)?;
        fixed_records::name_work(field.name(), group.column(), budget)?;
        if !field.name().eq_ignore_ascii_case(group.column()) {
            continue;
        }
        for update in group.updates() {
            check(budget, cancelled)?;
            if update.row() != ordinal {
                continue;
            }
            let before = cell(update.expected())?;
            let after = cell(update.value())?;
            if before != original || replacement.replace(after).is_some() {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(replacement.unwrap_or(original))
}

fn cell(value: &Value) -> Result<Option<i32>, EditError> {
    match value {
        Value::Null => Ok(None),
        Value::Int32(value) => Ok(Some(*value)),
        _ => Err(EditError::Preservation),
    }
}
