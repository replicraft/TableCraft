//! A populated source is qualified by real empty-history and INSERT replay.
use super::{FieldRenameError as Error, source::Qualified};
use crate::{
    FileFormat, InsertFieldValue, NativeImage, OmittedRowInsert,
    creation::FieldHistoryOffSource,
    format::{
        bytes::u32_at,
        data_type::INT32,
        layout::RECORD_COUNT_OFFSET,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        row::{ROW_COUNT_OFFSET, reference_page, reference_slot},
    },
    header::check,
    table::{
        indexes::page::{ENTRY_START, LEAF, PAGE_VERSION},
        mutation::{fixed_records, normal_ordinary, saved_field::Observed},
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const SOURCE_PAGES: usize = 11;
const DEFINITION: usize = 7;
const DATA: usize = 8;
const INDEX: usize = 9;
const LAST_ISSUED: usize = 20;
const UNIQUE_HISTORY: usize = 67;
const FREE: usize = 2;
const DATA_HEADER_BYTES: usize = ROW_COUNT_OFFSET + size_of::<u16>();

pub(super) fn capture(
    image: NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Qualified, Error> {
    validate(&image, table, budget, cancelled)?;
    let observed = Observed::capture(&image, table, column, budget, cancelled)?;
    if observed.identity.column_number() != 1 {
        return Err(Error::Source);
    }
    Ok(Qualified::from_populated(image, observed))
}

pub(super) fn validate(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    if image.catalog().header().format() != FileFormat::Ace2016
        || image.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES
    {
        return Err(Error::Source);
    }
    let source = normal_ordinary::capture_insert(image, table, budget, cancelled)?;
    let schema = source.table.schema();
    let fields = schema.columns();
    let rows = source.table.rows();
    if schema.definition_page() != DEFINITION as u32
        || rows.is_empty()
        || rows.len() != schema.declared_rows() as usize
        || source.records.len() != rows.len()
        || fields.len() != 2
        || fields.iter().enumerate().any(|(number, field)| {
            field.number() != number
                || field.native_type() != INT32
                || !field.fixed()
                || field.storage_length() != size_of::<i32>()
        })
        || source.indexes.indexes().len() != 1
    {
        return Err(Error::Source);
    }
    let index = &source.indexes.indexes()[0];
    if index.root_page() != INDEX as u32
        || index.columns().len() != 1
        || index.columns()[0].number() != 0
    {
        return Err(Error::Source);
    }
    budget.charge_allocation(rows.len() as u64, size_of::<OmittedRowInsert>() as u64)?;
    let mut requests = Vec::new();
    requests
        .try_reserve_exact(rows.len())
        .map_err(|_| crate::InspectError::Allocation)?;
    for (ordinal, (row, record)) in rows.iter().zip(&source.records).enumerate() {
        check(budget, cancelled)?;
        let id = i32::try_from(ordinal + 1).map_err(|_| Error::Source)?;
        if row.len() != 2
            || row[0] != Value::Int32(id)
            || reference_page(record.reference) != DATA as u32
            || usize::from(reference_slot(record.reference)) != ordinal
        {
            return Err(Error::Source);
        }
        let value = match row[1] {
            Value::Int32(value) => Value::Int32(value),
            Value::Null => Value::Null,
            _ => return Err(Error::Source),
        };
        budget.charge_allocation(2, size_of::<InsertFieldValue>() as u64)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(2)
            .map_err(|_| crate::InspectError::Allocation)?;
        values.push(InsertFieldValue::Omitted);
        values.push(InsertFieldValue::Supplied(value));
        requests.push(OmittedRowInsert::new(values));
    }
    let count = u32::try_from(rows.len()).map_err(|_| Error::Source)?;
    for offset in [RECORD_COUNT_OFFSET, LAST_ISSUED, UNIQUE_HISTORY] {
        if u32_at(image.as_bytes(), DEFINITION * PAGE_BYTES + offset)? != count {
            return Err(Error::Source);
        }
    }
    drop(source);

    // This mutable inverse is only a hypothesis. Complete old history/OFF proof
    // and real INSERT replay below must recover every original populated byte.
    let mut hypothesis = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    for offset in [RECORD_COUNT_OFFSET, LAST_ISSUED, UNIQUE_HISTORY] {
        hypothesis
            [DEFINITION * PAGE_BYTES + offset..DEFINITION * PAGE_BYTES + offset + size_of::<u32>()]
            .fill(0);
    }
    for (page, kind, version, free) in [
        (
            DATA,
            DATA_SIGNATURE[0],
            DATA_SIGNATURE[1],
            PAGE_BYTES - DATA_HEADER_BYTES,
        ),
        (INDEX, LEAF, PAGE_VERSION, PAGE_BYTES - ENTRY_START),
    ] {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, PAGE_BYTES as u64)?;
        let bytes = &mut hypothesis[page * PAGE_BYTES..(page + 1) * PAGE_BYTES];
        bytes.fill(0);
        bytes[0] = kind;
        bytes[1] = version;
        bytes[FREE..FREE + size_of::<u16>()].copy_from_slice(&(free as u16).to_le_bytes());
        bytes[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()]
            .copy_from_slice(&(DEFINITION as u32).to_le_bytes());
    }
    let scope = image.catalog().catalog().scope();
    let empty = NativeImage::capture(&mut Cursor::new(hypothesis), scope, budget, cancelled)?;
    let empty = FieldHistoryOffSource::capture(empty, budget, cancelled)?.into_image();
    let replay = empty.plan_ordinary_omitted_row_inserts(table, &requests, budget, cancelled)?;
    if replay.after_bytes().len() != image.as_bytes().len() {
        return Err(Error::Source);
    }
    for (original, reconstructed) in image
        .as_bytes()
        .chunks(PAGE_BYTES)
        .zip(replay.after_bytes().chunks(PAGE_BYTES))
    {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            (original.len() + reconstructed.len()) as u64,
        )?;
        if original != reconstructed {
            return Err(Error::Source);
        }
    }
    Ok(())
}
