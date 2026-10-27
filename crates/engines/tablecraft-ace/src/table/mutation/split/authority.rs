//! Original requests and selected metadata, never staged patches, grant row scope.
use super::super::EditError;
use super::super::physical::Patch;
use super::binding::{self, Binding};
use crate::{
    InspectError, NativeImage, TableCheck,
    format::reader::Reader,
    header::check,
    inspect_table_schema,
    table::indexes::{IndexKeyUpdate, MutationKey},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn prove<const WIDTH: usize>(
    image: &NativeImage,
    operation: &impl Binding<WIDTH>,
    checked: &TableCheck,
    actual: &[Patch<WIDTH>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexKeyUpdate>, EditError> {
    let table = operation.table();

    check(budget, cancelled)?;
    // Rebind metadata to the immutable operation table/column rather than a
    // mutable staged position or a supplied key-update vector.
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    if schema.definition != checked.schema().definition
        || schema.object_id() != checked.schema().object_id()
    {
        return Err(EditError::Preservation);
    }
    let (position, selected_kind) = binding::position(operation, &schema)?;
    let expected = operation.patches(
        &mut Reader {
            source: &mut image.reader(),
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        },
        &schema,
        position,
    )?;
    if expected.len() != actual.len() {
        return Err(EditError::Preservation);
    }
    for (original, supplied) in expected.iter().zip(actual) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (2 * WIDTH) as u64)
            .map_err(InspectError::from)?;
        if original.reference != supplied.reference
            || original.offset != supplied.offset
            || original.before != supplied.before
            || original.after != supplied.after
        {
            return Err(EditError::Preservation);
        }
    }
    let indexed = checked.indexes().iter().any(|index| {
        index
            .index()
            .columns()
            .iter()
            .any(|key| key.number() == schema.columns[position].number())
    });
    let mut keys = Vec::new();
    if indexed {
        let kind = MutationKey::new(selected_kind)?;
        budget
            .charge_allocation(expected.len() as u64, size_of::<IndexKeyUpdate>() as u64)
            .map_err(InspectError::from)?;
        keys.try_reserve_exact(expected.len())
            .map_err(|_| InspectError::Allocation)?;
        for patch in expected {
            check(budget, cancelled)?;
            keys.push(IndexKeyUpdate {
                reference: patch.reference,
                column: position,
                expected: Some(kind.stored(&patch.before)?),
                value: Some(kind.stored(&patch.after)?),
            });
        }
    }
    Ok(keys)
}
