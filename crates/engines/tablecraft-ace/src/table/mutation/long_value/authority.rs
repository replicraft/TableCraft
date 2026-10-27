//! Final authority is rebound from the immutable original operation and image.
use super::super::{EditError, page_update::roots};
use super::{
    Operation,
    admission::{self, Prepared},
    maps::{Family, Maps},
    ownership::Inventory,
    stage::Staged,
    value,
};
use crate::{
    InspectError, NativeImage, format::reader::Reader, header::check, limits::MAX_LIVE_ROWS,
    table::record,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn prove(
    image: &NativeImage,
    operation: &Operation<'_>,
    staged: &Staged<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, EditError> {
    check(budget, cancelled)?;
    if operation.updates.len() > MAX_LIVE_ROWS {
        return Err(EditError::Preservation);
    }
    let prepared = admission::prepare(image, operation.table, operation.column, budget, cancelled)?;
    let schema = prepared.structural.schema();
    let column = prepared.column;
    let number = schema.columns()[column].number();
    let references = prepared
        .storage
        .columns()
        .iter()
        .find(|report| report.column_number() == number)
        .and_then(|report| report.maps())
        .ok_or(EditError::Preservation)?;
    let maps = Maps::capture(image, schema, references, budget, cancelled)?;
    Inventory::capture(image, budget, cancelled)?.require_exclusive(
        schema,
        number,
        maps.pages(Family::LongValue),
        budget,
        cancelled,
    )?;
    let captured = roots::capture(image, schema, operation.updates, budget, cancelled)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut changed = 0;
    for original in captured.selected {
        reader.tick()?;
        let values = record::decode_selected(
            &original.final_record.data,
            schema,
            Some(&[column]),
            &mut reader,
        )?;
        if values.len() != 1
            || !value::equal(
                &values[0],
                original.update.expected(),
                reader.budget,
                cancelled,
            )?
        {
            return Err(EditError::Preservation);
        }
        reader.work(staged.changes.len())?;
        let mut matches = staged
            .changes
            .iter()
            .filter(|change| change.root.reference == original.reference);
        if value::equal(
            original.update.expected(),
            original.update.value(),
            reader.budget,
            cancelled,
        )? {
            if matches.next().is_some() {
                return Err(EditError::Preservation);
            }
            continue;
        }

        let change = matches.next().ok_or(EditError::Preservation)?;
        if matches.next().is_some()
            || change.root.word != original.word
            || change.root.range != original.range
            || change.root.final_record.offset != original.final_record.offset
            || change.root.update.row() != original.update.row()
        {
            return Err(EditError::Preservation);
        }
        reader
            .budget
            .charge(
                Resource::Steps,
                (original.final_record.data.len() * 2) as u64,
            )
            .map_err(InspectError::from)?;
        if change.root.final_record.data != original.final_record.data
            || !value::equal(
                change.root.update.expected(),
                original.update.expected(),
                reader.budget,
                cancelled,
            )?
            || !value::equal(
                change.root.update.value(),
                original.update.value(),
                reader.budget,
                cancelled,
            )?
        {
            return Err(EditError::Preservation);
        }
        changed += 1;
    }
    if changed != staged.changes.len() {
        return Err(EditError::Preservation);
    }

    check(reader.budget, cancelled)?;
    Ok(prepared)
}
