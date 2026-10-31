use super::{Error, tick};
use crate::commands::tsv;
use csv::{ByteRecord, ReaderBuilder};
use std::path::Path;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_INPUT_BYTES: u64 = 64 * 1024;
const MAX_ROWS: usize = 64;
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
    if length > MAX_INPUT_BYTES {
        return Err(Error::Input(
            "Creation TSV exceeds the 64 KiB input ceiling",
        ));
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
    if bytes.is_empty() || bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(Error::Input(
            "Expected a nonempty bounded TSV with a column header",
        ));
    }
    tsv::validate(bytes, budget, cancelled).map_err(|error| match error {
        tsv::Error::Native(error) => Error::Native(error),
        tsv::Error::Malformed(message) => Error::Input(message),
    })?;
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
    for _ in 0..CSV_RETAINED_RECORDS {
        budget
            .charge_allocation(bytes.len() as u64, 1)
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
    let mut record = ByteRecord::with_capacity(bytes.len(), fields);
    if !reader
        .read_byte_record(&mut record)
        .map_err(|_| Error::Input("Malformed creation TSV header"))?
        || record.len() != columns.len()
    {
        return Err(Error::Input(
            "TSV header must match the supplied columns exactly",
        ));
    }
    for (actual, expected) in record.iter().zip(columns) {
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
        if rows.len() == MAX_ROWS {
            return Err(Error::Input("Creation accepts at most 64 TSV rows"));
        }
        if record.len() != columns.len() {
            return Err(Error::Input(
                "Creation TSV rows must match the column count",
            ));
        }
        budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        let mut row = Vec::new();
        reserve(&mut row, columns.len(), budget)?;
        for cell in &record {
            tick(budget, cancelled)?;
            budget
                .charge(Resource::Steps, cell.len() as u64)
                .map_err(InspectError::from)?;
            let digits = cell.strip_prefix(b"-").unwrap_or(cell);
            if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
                return Err(Error::Input(
                    "Expected a present decimal Int32 with optional minus sign",
                ));
            }
            let text =
                std::str::from_utf8(cell).map_err(|_| Error::Input("Int32 cell is not ASCII"))?;
            row.push(
                text.parse::<i32>()
                    .map_err(|_| Error::Input("Creation Int32 cell is outside its signed range"))?,
            );
        }
        reserve(&mut rows, 1, budget)?;
        rows.push(row);
    }
    tick(budget, cancelled)?;
    Ok(rows)
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
