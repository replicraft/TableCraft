//! Independently reconstruct original retained rows and complete nullable keys.
use super::{Family, bind, source};
use crate::preservation::COPY_CHUNK_BYTES;
use crate::table::{
    indexes::{check_table_retained, mutation::normal_index_audit},
    mutation::{
        fixed_records::{self, indexed::i32_updates::I32UpdateIndexes},
        preserved_source::validation,
        rows::Directory,
    },
};
use crate::{
    EditError, FittedColumnUpdates, IndexCheckStatus, InspectError, NativeImage, header::check,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) fn prove(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    after: &[u8],
    returned_count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let source = source::capture(image, table, budget, cancelled)?;
    source
        .facts()
        .require_preserved_definition_tail(image, after, budget, cancelled)?;
    let requests = bind(image, &source, groups, budget, cancelled)?;
    let facts = I32UpdateIndexes::capture(image, table, source.facts(), budget, cancelled)?;
    let mut expected = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    let mut changed_rows = 0_usize;
    for request in &requests {
        check(budget, cancelled)?;
        if !request.changed {
            continue;
        }
        let original = source
            .facts()
            .records
            .get(request.row)
            .ok_or(EditError::Conflict)?;
        let record = Family::Retained.reconstruct_record(
            source.facts().table.schema(),
            request.column,
            &original.data,
            request.value,
            budget,
            cancelled,
        )?;
        if record.len() != original.data.len() {
            return Err(EditError::Preservation);
        }
        super::apply(
            &mut expected,
            original.offset,
            &original.data,
            &record,
            budget,
            cancelled,
        )?;
        changed_rows = changed_rows
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?;
    }
    let [
        FittedColumnUpdates::Fixed {
            column: name,
            updates,
        },
    ] = groups
    else {
        return Err(EditError::Preservation);
    };
    let updates = *updates;
    let column = source::column(&source, name, budget, cancelled)?;
    let mut directory = Directory::new(updates, budget, cancelled)?;
    directory.require_unique(budget, cancelled)?;
    normal_index_audit::reconstruct_retained_long(
        image,
        facts.checked(),
        &mut expected,
        |row, position, original, work| {
            if position != column {
                return Ok(original);
            }
            if let Some(position) = directory.at(row, work, cancelled)? {
                let update = &updates[position];
                let before = cell(update.expected())?;
                if before != original {
                    return Err(EditError::Conflict);
                }
                return cell(update.value());
            }
            Ok(original)
        },
        budget,
        cancelled,
    )?;
    validation::expected(
        image,
        table,
        &source.facts().table,
        &expected,
        budget,
        cancelled,
    )?;
    let checked = check_table_retained(
        &mut Cursor::new(&expected),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    if checked.schema().object_id() != source.facts().table.schema().object_id()
        || checked.schema().definition_page() != source.facts().table.schema().definition_page()
        || checked.rows() != source.facts().records.len()
        || checked.indexes().len() != facts.checked().indexes().len()
        || checked
            .indexes()
            .iter()
            .any(|entry| entry.status() != IndexCheckStatus::Checked)
        || changed_rows != returned_count
        || expected.len() != after.len()
    {
        return Err(EditError::Preservation);
    }
    for (expected, actual) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(after.chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != actual {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

fn cell(value: &Value) -> Result<Option<i32>, EditError> {
    match value {
        Value::Null => Ok(None),
        Value::Int32(value) => Ok(Some(*value)),
        _ => Err(EditError::Preservation),
    }
}
