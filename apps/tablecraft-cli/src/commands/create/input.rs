use super::{Error, schema, tick};
use crate::commands::{tsv, typed_values};
use csv::{ByteRecord, ReaderBuilder};
use std::path::Path;
use tablecraft_ace::InspectError;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

// The current native creator uses fixed 64-row layout/key proof storage.
// This is its qualified construction scope, not a TSV or Access format maximum.
const QUALIFIED_CREATION_ROWS: usize = 64;
const CSV_BUFFER_BYTES: usize = 8192;
const CSV_RETAINED_RECORDS: usize = 3;

pub(super) fn capture(
    path: &Path,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    tick(budget, cancelled)?;
    let mut file =
        tablecraft_io::open_regular_readonly(path).map_err(|error| Error::File(error.kind()))?;
    let length =
        tablecraft_io::seek_to_end(&mut file, budget, cancelled).map_err(InspectError::from)?;
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
        return Err(Error::Input("Creation TSV length changed during capture"));
    }
    Ok(bytes)
}

pub(super) fn rows(
    bytes: &[u8],
    columns: &[String],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<i32>>, Error> {
    rows_with(
        bytes,
        columns.len(),
        |position| columns[position].as_str(),
        |_, cell, _| int32(cell),
        budget,
        cancelled,
    )
}

pub(super) fn typed_rows(
    bytes: &[u8],
    fields: &[schema::Field<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Value>>, Error> {
    rows_with(
        bytes,
        fields.len(),
        |position| fields[position].column.name(),
        |position, cell, budget| {
            let text = std::str::from_utf8(cell)
                .map_err(|_| Error::Input("Typed creation cell is not UTF-8"))?;
            typed_values::update(fields[position].kind, text, budget).map_err(Error::from)
        },
        budget,
        cancelled,
    )
}

fn rows_with<'a, T>(
    bytes: &[u8],
    count: usize,
    name: impl Fn(usize) -> &'a str,
    parse: impl Fn(usize, &[u8], &mut ResourceBudget) -> Result<T, Error>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<T>>, Error> {
    if bytes.is_empty() {
        return Err(Error::Input("Expected a nonempty TSV with a column header"));
    }
    let layout = tsv::validate(bytes, budget, cancelled).map_err(|error| match error {
        tsv::Error::Native(error) => Error::Native(error),
        tsv::Error::Malformed(message) => Error::Input(message),
    })?;
    let fields = layout.max_fields;
    budget
        .charge_allocation(CSV_BUFFER_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    for _ in 0..CSV_RETAINED_RECORDS {
        budget
            .charge_allocation(layout.max_record_bytes as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge_allocation(fields as u64, size_of::<usize>() as u64)
            .map_err(InspectError::from)?;
    }
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .buffer_capacity(CSV_BUFFER_BYTES)
        .from_reader(bytes);
    let mut record = ByteRecord::with_capacity(layout.max_record_bytes, fields);
    if !reader
        .read_byte_record(&mut record)
        .map_err(|_| Error::Input("Malformed creation TSV header"))?
        || record.len() != count
    {
        return Err(Error::Input(
            "TSV header must match the supplied columns exactly",
        ));
    }
    for (position, actual) in record.iter().enumerate() {
        let expected = name(position);
        tick(budget, cancelled)?;
        budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if actual != expected.as_bytes() {
            return Err(Error::Input(
                "TSV header must match the supplied columns exactly",
            ));
        }
    }
    let mut rows = Vec::new();
    while reader
        .read_byte_record(&mut record)
        .map_err(|_| Error::Input("Malformed creation TSV record"))?
    {
        tick(budget, cancelled)?;
        if rows.len() == QUALIFIED_CREATION_ROWS {
            return Err(Error::Input("Creation accepts at most 64 TSV rows"));
        }
        if record.len() != count {
            return Err(Error::Input(
                "Creation TSV rows must match the column count",
            ));
        }
        budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        let mut row = Vec::new();
        reserve(&mut row, count, budget)?;
        for (position, cell) in record.iter().enumerate() {
            tick(budget, cancelled)?;
            budget
                .charge(Resource::Steps, cell.len() as u64)
                .map_err(InspectError::from)?;
            row.push(parse(position, cell, budget)?);
        }
        reserve(&mut rows, 1, budget)?;
        rows.push(row);
    }
    tick(budget, cancelled)?;
    Ok(rows)
}

fn int32(cell: &[u8]) -> Result<i32, Error> {
    let digits = cell.strip_prefix(b"-").unwrap_or(cell);
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(Error::Input(
            "Expected a present decimal Int32 with optional minus sign",
        ));
    }
    let text = std::str::from_utf8(cell).map_err(|_| Error::Input("Int32 cell is not ASCII"))?;
    text.parse::<i32>()
        .map_err(|_| Error::Input("Creation Int32 cell is outside its signed range"))
}

pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    budget: &mut ResourceBudget,
) -> Result<(), InspectError> {
    budget.charge_allocation(additional as u64, size_of::<T>() as u64)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| InspectError::Allocation)
}
