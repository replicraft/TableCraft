//! Complete long descriptors precede ordinary DATA and INDEX on one cursor.
mod maps;
mod placement;

use self::{
    maps::Maps,
    placement::{Builder, Column},
};
use super::record::input::{Input, Payload};
use crate::{
    EditError, InspectError, LongValueCheckStatus, NativeImage, TableCheck, WriteBlocker,
    check_table_long_values,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        reader::Reader,
    },
    header::check,
    table::mutation::{
        append::State,
        long_value::{encode::StorageChoice, maps::Scope, ownership::Inventory},
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Page {
    pub number: u32,
    pub column: usize,
    pub owned: u32,
    pub candidate: u32,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

pub(super) struct BitmapPage {
    pub number: u32,
    pub column: usize,
    pub owned: u32,
    pub block: u32,
    pub bytes: Vec<u8>,
}

pub(super) struct Plan {
    pub pages: Vec<Page>,
    pub metadata: Vec<BitmapPage>,
}

/// All request payloads and row-capacity choices have already been validated.
/// Resolve only final choices; no provisional descriptor enters a DATA page.
pub(super) fn prepare(
    image: &NativeImage,
    checked: &TableCheck,
    variables: &[usize],
    inputs: &mut [Input],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Plan, State), EditError> {
    check(budget, cancelled)?;
    let mut selected = Vec::new();
    reserve(&mut selected, variables.len(), budget)?;
    for input in inputs.iter() {
        if input.payloads.len() != variables.len() {
            return Err(EditError::Preservation);
        }
        for (ordinal, payload) in input.payloads.iter().enumerate() {
            check(budget, cancelled)?;
            if matches!(
                payload,
                Payload::Long {
                    choice: StorageChoice::Single | StorageChoice::Chained,
                    ..
                }
            ) {
                let column = variables[ordinal];
                budget
                    .charge(
                        tablecraft_core::limits::Resource::Steps,
                        selected.len() as u64,
                    )
                    .map_err(InspectError::from)?;
                if !selected.contains(&column) {
                    selected.push(column);
                }
            }
        }
    }
    let append = State::new(image, checked.schema().definition_page());
    let plan = Plan {
        pages: Vec::new(),
        metadata: Vec::new(),
    };
    if selected.is_empty() {
        for input in inputs {
            for payload in &mut input.payloads {
                if let Payload::Long { encoded, choice } = payload {
                    let bytes = encoded.descriptor(*choice, None, budget, cancelled)?;
                    *payload = Payload::Bytes(bytes);
                }
            }
        }
        return Ok((plan, append));
    }
    let schema = checked.schema();
    let report = check_table_long_values(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if !schema.same_definition(report.schema()) {
        return Err(EditError::Preservation);
    }
    let inventory = Inventory::capture(image, budget, cancelled)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(&mut reader, &global)?;
    let mut columns = Vec::new();
    reserve(&mut columns, selected.len(), reader.budget)?;
    for position in selected {
        reader.tick()?;
        let field = schema
            .columns()
            .get(position)
            .ok_or(EditError::Preservation)?;
        reader.work(report.columns().len())?;
        let stored = report
            .columns()
            .iter()
            .find(|column| column.column_number() == field.number())
            .ok_or(EditError::Preservation)?;
        if stored.status() != LongValueCheckStatus::Checked
            || stored.shared_references() != 0
            || stored.shared_owned_pages() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let references = stored.maps().ok_or(EditError::Preservation)?;
        let scope = Scope::capture(
            &mut reader,
            references.owned(),
            references.candidate(),
            MapRole::LongValueCandidate,
            &global,
        )?;
        inventory.require_exclusive(
            schema,
            field.number(),
            &scope.pages,
            reader.budget,
            reader.cancelled,
        )?;
        columns.push(Column {
            position,
            number: field.number(),
            references,
            maps: Maps::capture(scope, field.number(), references, &mut reader)?,
        });
    }
    let mut builder = Builder {
        image,
        schema,
        inventory,
        global,
        columns,
        plan,
        append,
    };
    for input in inputs {
        for (ordinal, payload) in input.payloads.iter_mut().enumerate() {
            reader.tick()?;
            let Payload::Long { encoded, choice } = payload else {
                continue;
            };
            let mut first = None;
            let chunks = encoded.chunks(*choice, reader.budget, reader.cancelled)?;
            let mut placed = Vec::new();
            reserve(&mut placed, chunks.len(), reader.budget)?;
            for chunk in &chunks {
                let body = encoded.body(chunk, 0, reader.budget, reader.cancelled)?;
                let location = builder.append(variables[ordinal], &body, &mut reader)?;
                if first.is_none() {
                    first = Some(location.reference);
                }
                placed.push(location);
            }
            for (position, chunk) in chunks.iter().enumerate() {
                reader.tick()?;
                if chunk.linked {
                    let next = placed
                        .get(position + 1)
                        .map_or(0, |location| location.reference);
                    builder.link(&placed[position], next, &mut reader)?;
                }
            }
            let descriptor = encoded.descriptor(*choice, first, reader.budget, reader.cancelled)?;
            *payload = Payload::Bytes(descriptor);
        }
    }
    check(reader.budget, reader.cancelled)?;
    Ok((builder.plan, builder.append))
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    values
        .try_reserve(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}
