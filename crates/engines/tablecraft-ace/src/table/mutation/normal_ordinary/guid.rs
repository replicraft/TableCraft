//! Selected manual GUID-index UPDATE under original complete native authority.
mod proof;
use super::indexed::apply;
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage,
    format::data_type::GUID,
    header::check,
    table::{
        indexes::{IndexKeyUpdate, mutation::normal_guid_update},
        mutation::{
            fixed_records::{self, binding, indexed::guid_updates::GuidUpdateIndexes, stage},
            patch::BytePatch,
            preserved_source::{Source, validation},
        },
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Guid, Value},
};

const KEY_CHANGE_PASSES: usize = 6;

pub(super) fn plan(
    image: NativeImage,
    source: Source,
    table: &str,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let facts = GuidUpdateIndexes::capture(&image, &source, budget, cancelled)?;
    let (bound, _) = binding::all_guid_updates(&image, &source, groups, &facts, budget, cancelled)?;
    // Every declaration/carrier is bound before any expected value is compared.
    for (group, target) in groups.iter().zip(&bound) {
        for update in group.updates() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, (2 * target.width).max(16) as u64)
                .map_err(InspectError::from)?;
            let original = source
                .table
                .rows()
                .get(update.row())
                .and_then(|row| row.get(target.column))
                .ok_or(EditError::Conflict)?;
            if original != update.expected() {
                return Err(EditError::Conflict);
            }
        }
    }
    let (data, changed_rows) =
        stage::prepare_guid_updates(&image, &source, groups, &facts, budget, cancelled)?;
    let updates = index_updates(&source, groups, &bound, budget, cancelled)?;
    let indexes = normal_guid_update::patches(
        &image,
        facts.checked(),
        &facts.ownership,
        &updates,
        budget,
        cancelled,
    )?;
    let mut after = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    for patch in data {
        apply(
            &mut after,
            patch.offset(),
            patch.before(),
            patch.after(),
            budget,
            cancelled,
        )?;
    }
    for patch in indexes {
        apply(
            &mut after,
            patch.offset,
            &patch.before,
            &patch.after,
            budget,
            cancelled,
        )?;
    }
    validation::expected(&image, table, &source.table, &after, budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    drop(facts);
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows,
    };
    proof::prove(&plan, scope, table, groups, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

fn index_updates(
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    bound: &[binding::Bound],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexKeyUpdate>, EditError> {
    let mut count = 0_usize;
    for group in groups {
        check(budget, cancelled)?;
        count = count
            .checked_add(group.updates().len())
            .ok_or(InspectError::StructureLimit)?;
    }
    budget
        .charge_allocation(count as u64, size_of::<IndexKeyUpdate>() as u64)
        .map_err(InspectError::from)?;
    let mut keys = Vec::new();
    keys.try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for (group, target) in groups.iter().zip(bound) {
        check(budget, cancelled)?;
        let field = source
            .table
            .schema()
            .columns()
            .get(target.column)
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
        if field.native_type() != GUID {
            return Err(EditError::Preservation);
        }
        for update in group.updates() {
            check(budget, cancelled)?;
            // Canonical GUID bytes enter the typed key service directly. The
            // native DATA permutation belongs only to fixed-record encoding.
            budget
                .charge(
                    Resource::Steps,
                    (Guid::BYTE_LEN * KEY_CHANGE_PASSES + size_of::<IndexKeyUpdate>()) as u64,
                )
                .map_err(InspectError::from)?;
            let expected = key(update.expected())?;
            let value = key(update.value())?;
            if expected == value {
                continue;
            }
            let reference = source
                .records
                .get(update.row())
                .ok_or(EditError::Conflict)?
                .reference;
            keys.push(IndexKeyUpdate::canonical_guid(
                reference,
                target.column,
                expected,
                value,
            ));
        }
    }
    Ok(keys)
}

fn key(value: &Value) -> Result<Option<[u8; Guid::BYTE_LEN]>, EditError> {
    match value {
        Value::Null => Ok(None),
        Value::Guid(value) => Ok(Some(value.to_bytes())),
        _ => Err(EditError::Preservation),
    }
}
