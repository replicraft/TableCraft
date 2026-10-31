//! Fresh original-request record authority, independent of staging and placement.
use super::super::super::{
    name_work,
    source::{self, Source},
};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, SlotScalarUpdate, WriteBlocker,
    format::reader::Reader,
    header::check,
    table::{
        mutation::{
            page_update::roots,
            slot::storage,
            variable_update::{map_audit::record::RecordImage, record_proof},
        },
        record,
    },
};
use std::ops::Range;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

/// Immutable complete requested bytes and original logical/physical identity.
/// Only this independent original-source reconstruction can construct a record.
pub(super) struct Record {
    reference: u32,
    root_word: u16,
    root_range: Range<usize>,
    final_offset: usize,
    original_length: usize,
    bytes: Vec<u8>,
}

impl RecordImage for Record {
    fn reference(&self) -> u32 {
        self.reference
    }

    fn root_word(&self) -> u16 {
        self.root_word
    }

    fn root_range(&self) -> &Range<usize> {
        &self.root_range
    }

    fn final_offset(&self) -> usize {
        self.final_offset
    }

    fn original_length(&self) -> usize {
        self.original_length
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub(super) struct Verified {
    source_address: usize,
    source_length: usize,
    pub(super) original: Source,
    pub(super) records: Vec<Record>,
}

impl Verified {
    pub(super) fn require_source(&self, image: &NativeImage) -> Result<(), EditError> {
        if self.source_address != image.as_bytes().as_ptr() as usize
            || self.source_length != image.as_bytes().len()
        {
            return Err(EditError::Preservation);
        }
        Ok(())
    }
}

pub(super) fn records(
    image: &NativeImage,
    table: &str,
    column: &str,
    updates: &[SlotScalarUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Verified, EditError> {
    check(budget, cancelled)?;
    if updates.len() > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS {
        return Err(InspectError::StructureLimit.into());
    }
    let original = source::capture(image, table, budget, cancelled)?;
    let schema = original.table.schema();
    name_work(schema.name(), table, budget)?;
    if !schema.name().eq_ignore_ascii_case(table) {
        return Err(EditError::Preservation);
    }

    let mut selected = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        name_work(field.name(), column, budget)?;
        if field.name().eq_ignore_ascii_case(column) && selected.replace(position).is_some() {
            return Err(EditError::Preservation);
        }
    }
    let selected = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let field = &schema.columns()[selected];
    if !field.is_ordinary_short_text() {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    storage::qualify(field)?;
    for dependency in &original.dependencies {
        check(budget, cancelled)?;
        if *dependency == field.number() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    for index in original.indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            if key.number() == field.number() {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
    }
    for update in updates {
        validate(field, update.expected(), budget, cancelled)?;
        validate(field, update.value(), budget, cancelled)?;
    }

    // Directory/native traversal establishes source live-ordinal order and
    // exclusive final-record ownership, including every selected no-op. Caller
    // order and producer projection/event caches supply no record authority.
    let captured = roots::capture(image, schema, updates, budget, cancelled)?;
    budget
        .charge_allocation(updates.len() as u64, size_of::<Record>() as u64)
        .map_err(InspectError::from)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for root in captured.selected {
        reader.tick()?;
        let cells = record::decode_selected(
            &root.final_record.data,
            schema,
            Some(&[selected]),
            &mut reader,
        )?;
        let [actual] = cells.as_slice() else {
            return Err(EditError::Preservation);
        };
        validate(field, actual, reader.budget, cancelled)?;
        if !equal(actual, root.update.expected(), reader.budget, cancelled)? {
            return Err(EditError::Conflict);
        }
        if equal(actual, root.update.value(), reader.budget, cancelled)? {
            // Equal text must not be recompressed, and absent text must retain
            // ignored payload bytes. No changed record or placement is allowed.
            continue;
        }

        // The existing independent serializer uses MinimumSize here. It does
        // not consume the producer's encoded payload or supplied-row composer.
        let bytes = record_proof::reconstruct(
            schema,
            selected,
            &root.final_record.data,
            root.update.value(),
            reader.budget,
            cancelled,
        )?;
        let final_cells = record::decode_selected(&bytes, schema, Some(&[selected]), &mut reader)?;
        let [final_value] = final_cells.as_slice() else {
            return Err(EditError::Preservation);
        };
        if !equal(final_value, root.update.value(), reader.budget, cancelled)? {
            return Err(EditError::Preservation);
        }
        records.push(Record {
            reference: root.reference,
            root_word: root.word,
            root_range: root.range,
            final_offset: root.final_record.offset,
            original_length: root.final_record.data.len(),
            bytes,
        });
    }
    reader.tick()?;
    Ok(Verified {
        source_address: image.as_bytes().as_ptr() as usize,
        source_length: image.as_bytes().len(),
        original,
        records,
    })
}

fn validate(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if !matches!(value, Value::Text(_) | Value::Null) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    budget
        .charge(Resource::Steps, width(value))
        .map_err(InspectError::from)?;
    storage::validate(field, value)
}

fn equal(
    left: &Value,
    right: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    let work = width(left)
        .checked_add(width(right))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work)
        .map_err(InspectError::from)?;
    Ok(left == right)
}

fn width(value: &Value) -> u64 {
    match value {
        Value::Text(text) => text.len() as u64,
        _ => size_of::<Value>() as u64,
    }
}
