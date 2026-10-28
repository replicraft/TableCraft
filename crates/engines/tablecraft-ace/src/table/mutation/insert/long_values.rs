//! Complete long descriptors precede ordinary DATA and INDEX on one cursor.
mod initialization;
mod maps;
mod placement;

use self::{
    maps::Maps,
    placement::{Builder, Column},
};
use super::record::input::{Input, Payload};
use crate::{
    EditError, InspectError, LongValueCheckStatus, LongValueMapReferences, NativeImage, TableCheck,
    WriteBlocker,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        reader::Reader,
    },
    header::check,
    table::long_value::check_table_long_values_for_initialization,
    table::mutation::{
        append::State,
        long_value::{encode::StorageChoice, maps::Scope, ownership::Inventory},
    },
};
use tablecraft_core::limits::ResourceBudget;

const MAP_OWNED_SLOT: usize = 0;
const MAP_CANDIDATE_SLOT: usize = 1;
const MAX_PACKED_MAP_PAGE: u32 = u32::MAX >> u8::BITS;

fn packed_maps(number: u32) -> Result<LongValueMapReferences, EditError> {
    if number > MAX_PACKED_MAP_PAGE {
        return Err(InspectError::StructureLimit.into());
    }
    let page = number << u8::BITS;
    Ok(LongValueMapReferences {
        owned: page | MAP_OWNED_SLOT as u32,
        candidate: page | MAP_CANDIDATE_SLOT as u32,
    })
}

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

pub(super) struct Initialization {
    pub number: u32,
    pub column: usize,
    pub offset: usize,
    pub before: [u8; 2 * size_of::<u32>()],
    pub after: [u8; 2 * size_of::<u32>()],
    pub bytes: Vec<u8>,
}

pub(super) struct Plan {
    pub qualified_zero_columns: Vec<usize>,
    pub initializations: Vec<Initialization>,
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
    let mut append = State::new(image, checked.schema().definition_page());
    let mut plan = Plan {
        qualified_zero_columns: Vec::new(),
        initializations: Vec::new(),
        pages: Vec::new(),
        metadata: Vec::new(),
    };
    let schema = checked.schema();
    if selected.is_empty() && !has_zero_pair(image, schema, budget, cancelled)? {
        finish_inline(inputs, budget, cancelled)?;
        return Ok((plan, append));
    }
    let mut native_columns = Vec::new();
    reserve(&mut native_columns, schema.columns().len(), budget)?;
    for field in schema.columns() {
        check(budget, cancelled)?;
        if matches!(
            field.native_type(),
            crate::format::data_type::LONG_TEXT | crate::format::data_type::OLE
        ) {
            native_columns.push(field.number());
        }
    }
    let (inventory, zeros) =
        Inventory::capture_for_initialization(image, schema, &native_columns, budget, cancelled)?;
    reserve(&mut plan.qualified_zero_columns, zeros.len(), budget)?;
    for zero in &zeros {
        check(budget, cancelled)?;
        plan.qualified_zero_columns.push(zero.column);
    }
    if selected.is_empty() {
        finish_inline(inputs, budget, cancelled)?;
        return Ok((plan, append));
    }
    reader_order(&mut selected, schema, budget, cancelled)?;
    let report = check_table_long_values_for_initialization(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        &native_columns,
        budget,
        cancelled,
    )?;
    if !schema.same_definition(report.schema()) {
        return Err(EditError::Preservation);
    }
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
        reader.work(zeros.len())?;
        if let Some(zero) = zeros.iter().find(|zero| zero.column == field.number()) {
            let initialized =
                initialization::prepare(schema, zero, &global, &mut append, &mut reader)?;
            let references = packed_maps(initialized.number)?;
            reserve(&mut plan.initializations, 1, reader.budget)?;
            plan.initializations.push(initialized);
            columns.push(Column {
                position,
                number: field.number(),
                references,
                maps: Maps::fresh(field.number(), references, &mut reader)?,
            });
        } else {
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
    for initialization in &mut builder.plan.initializations {
        reader.work(builder.columns.len())?;
        let column = builder
            .columns
            .iter()
            .find(|column| column.number == initialization.column)
            .ok_or(EditError::Preservation)?;
        initialization.bytes = column.maps.initialized_bytes(&mut reader)?;
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

fn reader_order(
    selected: &mut [usize],
    schema: &crate::TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let work = selected
        .len()
        .checked_mul(selected.len())
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(tablecraft_core::limits::Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    selected.sort_unstable_by_key(|position| schema.columns()[*position].field.variable);
    check(budget, cancelled)?;
    Ok(())
}

/// Look only for an explicit zero pair. This does not grant eligibility: the
/// complete initialization parser and live descriptor census must still pass.
fn has_zero_pair(
    image: &NativeImage,
    schema: &crate::TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    budget
        .charge(
            tablecraft_core::limits::Resource::Steps,
            schema.columns().len() as u64,
        )
        .map_err(InspectError::from)?;
    if !schema.columns().iter().any(|field| {
        matches!(
            field.native_type(),
            crate::format::data_type::LONG_TEXT | crate::format::data_type::OLE
        )
    }) {
        return Ok(false);
    }
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let declarations = crate::table::long_value::maps::declarations(&mut reader, schema)?;
    reader.work(declarations.entries.len())?;
    Ok(declarations
        .entries
        .iter()
        .any(|entry| entry.references.owned() == 0 && entry.references.candidate() == 0))
}

fn finish_inline(
    inputs: &mut [Input],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for input in inputs {
        check(budget, cancelled)?;
        for payload in &mut input.payloads {
            check(budget, cancelled)?;
            if let Payload::Long { encoded, choice } = payload {
                let bytes = encoded.descriptor(*choice, None, budget, cancelled)?;
                *payload = Payload::Bytes(bytes);
            }
        }
    }
    Ok(())
}
