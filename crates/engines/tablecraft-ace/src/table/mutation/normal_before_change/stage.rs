//! Original-record fixed patches; all final values come from fresh preparation.
use super::super::{
    EditError, Int32Update, guards::buffer, physical, record_protection::Protection,
};
use crate::{
    BeforeChangeFieldPreparation, InspectError, NativeImage, TableCheck, TableSnapshot,
    WriteBlocker,
    format::reader::Reader,
    header::check,
    table::indexes::{IndexDataPatch, IndexKeyUpdate, MutationKey, mutation::normal_fitting},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

pub(super) struct Staged {
    pub patches: Vec<physical::Patch<{ size_of::<i32>() }>>,
    pub indexes: Vec<IndexDataPatch>,
    pub changed_rows: usize,
}

pub(super) fn derive(
    image: &NativeImage,
    checked: &TableCheck,
    snapshot: &TableSnapshot,
    prepared: &BeforeChangeFieldPreparation<'_>,
    protection: &Protection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    check(budget, cancelled)?;
    let schema = checked.schema();
    let indexes = checked.indexes();
    if prepared.source_bytes().as_ptr() != image.as_bytes().as_ptr()
        || prepared.source_bytes().len() != image.as_bytes().len()
        || prepared.table_object_id() != schema.object_id()
        || snapshot.schema().object_id() != schema.object_id()
    {
        return Err(EditError::Preservation);
    }
    let rows = prepared.final_rows().ok_or(EditError::Preservation)?;
    let maximum = rows
        .len()
        .checked_mul(schema.columns().len())
        .ok_or(InspectError::StructureLimit)?;
    let mut patches = buffer::vector(maximum, budget, cancelled)?;
    let mut keys = if indexes.is_empty() {
        Vec::new()
    } else {
        buffer::vector(maximum, budget, cancelled)?
    };
    for column in 0..schema.columns().len() {
        check(budget, cancelled)?;
        let mut updates = buffer::vector(rows.len(), budget, cancelled)?;
        for row in rows {
            check(budget, cancelled)?;
            if row.values().len() != schema.columns().len() {
                return Err(EditError::Preservation);
            }
            let original = snapshot
                .rows()
                .get(row.ordinal())
                .ok_or(EditError::Conflict)?;
            let (Some(Value::Int32(before)), Some(Value::Int32(after))) =
                (original.get(column), row.values().get(column))
            else {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            };
            if before != after {
                updates.push(Int32Update::new(row.ordinal(), *before, *after));
            }
        }
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let staged =
            physical::patches::<_, _, { size_of::<i32>() }>(&mut reader, schema, column, &updates)?;
        let mut indexed = false;
        for index in indexes {
            for key in index.index().columns() {
                check(reader.budget, cancelled)?;
                if key.number() == schema.columns()[column].number() {
                    indexed = true;
                }
            }
        }
        for patch in staged {
            protection.require(
                image,
                schema,
                patch.offset,
                size_of::<i32>(),
                reader.budget,
                cancelled,
            )?;
            if indexed {
                let kind = MutationKey::new(ScalarType::Int32)?;
                keys.push(IndexKeyUpdate {
                    reference: patch.reference,
                    column,
                    expected: Some(kind.stored(&patch.before)?),
                    value: Some(kind.stored(&patch.after)?),
                });
            }
            patches.push(patch);
        }
    }
    let work = patches
        .len()
        .checked_mul(patches.len().checked_ilog2().unwrap_or(0) as usize + 1)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    patches.sort_unstable_by_key(|patch| patch.offset);
    check(budget, cancelled)?;
    for pair in patches.windows(2) {
        check(budget, cancelled)?;
        if pair[0].offset + size_of::<i32>() > pair[1].offset {
            return Err(EditError::Preservation);
        }
    }
    let mut references = buffer::vector(patches.len(), budget, cancelled)?;
    for patch in &patches {
        check(budget, cancelled)?;
        references.push(patch.reference);
    }
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    references.sort_unstable();
    references.dedup();
    check(budget, cancelled)?;
    let indexes = if indexes.is_empty() {
        Vec::new()
    } else {
        // Apply all original-to-final field changes together. In particular,
        // unique swaps never pass through a partially changed key population.
        normal_fitting::patches(image, checked, &keys, budget, cancelled)?
    };
    Ok(Staged {
        patches,
        indexes,
        changed_rows: references.len(),
    })
}
