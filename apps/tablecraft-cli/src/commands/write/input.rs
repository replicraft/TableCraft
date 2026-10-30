//! Capture and fully parse explicit request inputs before opening a database.
pub(super) mod presence;
mod quotes;
use super::{
    error::{Error, input},
    tick, values,
};
use csv::{ByteRecord, ReaderBuilder};
use std::{fs::File, path::Path};
use tablecraft_ace::{InspectError, RowDelete};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const MAX_REQUESTS: usize = 4096;
pub(super) const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const DEFAULT_VALUE_BYTES: usize = 1024;
const CSV_BUFFER_BYTES: usize = 8192;
const CSV_RETAINED_RECORDS: usize = 3;
const VALUE_FIELDS: usize = 2;
const DELETE_FIELDS: usize = 2;
const MAX_RECORD_BYTES: usize = 4096;
const MAX_INPUT_COLUMNS: usize = 256;
const HEX_DIGITS_PER_BYTE: usize = 2;

pub(super) fn value_limit(text: &str) -> Result<usize, String> {
    let value = text
        .parse::<usize>()
        .map_err(|_| "Expected a positive value-byte limit".to_owned())?;
    if value == 0 || value > MAX_INPUT_BYTES as usize {
        return Err("Value-byte limit must be between 1 and 16777216".to_owned());
    }
    Ok(value)
}

pub(super) fn capture(
    path: &Path,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    tick(budget, cancelled)?;
    let mut file = File::open(path).map_err(|error| Error::File(error.kind()))?;
    let length =
        tablecraft_io::seek_to_end(&mut file, budget, cancelled).map_err(InspectError::from)?;
    if length > MAX_INPUT_BYTES {
        return Err(input("Request file exceeds the 16 MiB input ceiling"));
    }
    budget
        .charge_allocation(length, 1)
        .map_err(InspectError::from)?;
    let size = usize::try_from(length).map_err(|_| InspectError::StructureLimit)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| InspectError::Allocation)?;
    bytes.resize(size, 0);
    tablecraft_io::read_exact_at(&mut file, 0, &mut bytes, budget, cancelled)
        .map_err(InspectError::from)?;
    if tablecraft_io::seek_to_end(&mut file, budget, cancelled).map_err(InspectError::from)?
        != length
    {
        return Err(input("Request file length changed during capture"));
    }
    Ok(bytes)
}

fn reader<'a>(
    bytes: &'a [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(csv::Reader<&'a [u8]>, ByteRecord), Error> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(input(
            "Expected a nonempty request file within the input ceiling",
        ));
    }
    quotes::validate(bytes, budget, cancelled)?;
    let fields = bytes
        .len()
        .checked_add(1)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(CSV_BUFFER_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    // csv retains first-record byte/string caches even without headers. Account
    // for these and the reused record, including delimiter-only hostile input.
    for _ in 0..CSV_RETAINED_RECORDS {
        budget
            .charge_allocation(bytes.len() as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge_allocation(fields as u64, size_of::<usize>() as u64)
            .map_err(InspectError::from)?;
    }
    let reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .buffer_capacity(CSV_BUFFER_BYTES)
        .from_reader(bytes);
    Ok((reader, ByteRecord::with_capacity(bytes.len(), fields)))
}

fn insert_fields<F, T>(
    bytes: &[u8],
    max_value_bytes: usize,
    parse: fn(&str, &str, &mut ResourceBudget) -> Result<F, Error>,
    make: fn(Vec<F>) -> T,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, Error> {
    if max_value_bytes == 0 || max_value_bytes > MAX_INPUT_BYTES as usize {
        return Err(input("Invalid value-byte limit"));
    }
    let (mut reader, mut record) = reader(bytes, budget, cancelled)?;
    let mut rows = Vec::new();
    let mut arity = None;
    while reader
        .read_byte_record(&mut record)
        .map_err(|_| input("Malformed quoted TSV request"))?
    {
        tick(budget, cancelled)?;
        if rows.len() == MAX_REQUESTS
            || record.is_empty()
            || !record.len().is_multiple_of(VALUE_FIELDS)
            || record.len() / VALUE_FIELDS > MAX_INPUT_COLUMNS
        {
            return Err(input(
                "Expected at most 4096 complete KIND/DATA row requests",
            ));
        }
        if arity.is_some_and(|prior| prior != record.len()) {
            return Err(input("Insert rows have inconsistent field counts"));
        }
        arity = Some(record.len());
        budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        let mut fields = Vec::new();
        reserve(&mut fields, record.len() / VALUE_FIELDS, budget)?;
        for position in (0..record.len()).step_by(VALUE_FIELDS) {
            tick(budget, cancelled)?;
            let kind =
                std::str::from_utf8(&record[position]).map_err(|_| input("KIND is not UTF-8"))?;
            let data = std::str::from_utf8(&record[position + 1])
                .map_err(|_| input("DATA is not UTF-8"))?;
            if data.len() > max_value_bytes {
                return Err(input("A value exceeds the requested value-byte limit"));
            }
            fields.push(parse(kind, data, budget)?);
        }
        reserve(&mut rows, 1, budget)?;
        rows.push(make(fields));
    }
    if rows.is_empty() {
        return Err(input("Request file contains no rows"));
    }
    tick(budget, cancelled)?;
    Ok(rows)
}

pub(super) fn delete_inline(text: &str, budget: &mut ResourceBudget) -> Result<RowDelete, Error> {
    let (ordinal, hex) = text
        .split_once(':')
        .ok_or_else(|| input("Expected ROW:HEX for --delete"))?;
    delete(ordinal, hex, budget)
}

fn delete(ordinal: &str, hex: &str, budget: &mut ResourceBudget) -> Result<RowDelete, Error> {
    if hex.is_empty() || hex.len() / HEX_DIGITS_PER_BYTE > MAX_RECORD_BYTES {
        return Err(input(
            "Expected a nonempty original record of at most 4096 bytes",
        ));
    }
    let ordinal = ordinal
        .parse()
        .map_err(|_| input("ROW must be a nonnegative live-row ordinal"))?;
    budget
        .charge(Resource::Rows, 1)
        .map_err(InspectError::from)?;
    Ok(RowDelete::new(ordinal, values::hex(hex, budget)?))
}

pub(super) fn deletes(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowDelete>, Error> {
    let (mut reader, mut record) = reader(bytes, budget, cancelled)?;
    let mut rows = Vec::new();
    while reader
        .read_byte_record(&mut record)
        .map_err(|_| input("Malformed quoted DELETE TSV"))?
    {
        tick(budget, cancelled)?;
        if rows.len() == MAX_REQUESTS || record.len() != DELETE_FIELDS {
            return Err(input("Expected at most 4096 ROW/HEX delete requests"));
        }
        let ordinal = std::str::from_utf8(&record[0]).map_err(|_| input("Invalid ROW text"))?;
        let hex = std::str::from_utf8(&record[1]).map_err(|_| input("Invalid HEX text"))?;
        reserve(&mut rows, 1, budget)?;
        rows.push(delete(ordinal, hex, budget)?);
    }
    if rows.is_empty() {
        return Err(input("Request file contains no rows"));
    }
    tick(budget, cancelled)?;
    Ok(rows)
}

pub(super) fn reserve<T>(
    items: &mut Vec<T>,
    count: usize,
    budget: &mut ResourceBudget,
) -> Result<(), Error> {
    let required = items
        .len()
        .checked_add(count)
        .filter(|count| *count <= MAX_REQUESTS)
        .ok_or(InspectError::StructureLimit)?;
    if required <= items.capacity() {
        return Ok(());
    }
    let capacity = required.max(items.capacity().saturating_mul(2).min(MAX_REQUESTS));
    let growth = capacity - items.capacity();
    budget
        .charge_allocation(growth as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let copied = items
        .len()
        .checked_mul(size_of::<T>())
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, copied as u64)
        .map_err(InspectError::from)?;
    items
        .try_reserve_exact(capacity - items.len())
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}
