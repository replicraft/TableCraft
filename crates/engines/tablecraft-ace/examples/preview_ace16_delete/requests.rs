//! Bounded application input; physical record validation stays in the engine.
use clap::Args;
use csv::{ReaderBuilder, StringRecord};
use std::{error::Error, fs::File, path::PathBuf};
use tablecraft_ace::RowDelete;
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_REQUEST_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REQUEST_ROWS: usize = 4096;
const MAX_EXPECTED_ROW_BYTES: usize = 4096;
const READER_BUFFER_BYTES: usize = 8 * 1024;
const HEX_DIGITS_PER_BYTE: usize = 2;
const REQUEST_COLUMNS: usize = 2;
const CSV_RETAINED_RECORDS: usize = 3;

#[derive(Clone)]
struct DeleteArgument {
    row: usize,
    expected: Vec<u8>,
}

#[derive(Args)]
#[group(id = "deletion_requests", required = true, multiple = false)]
pub(super) struct Requests {
    /// Zero-based live-row ordinal and exact original record payload in hexadecimal.
    /// Includes column-count header and null mask, excluding page/slot words.
    #[arg(long = "delete", value_name = "ROW:HEX", value_parser = parse_inline)]
    deletes: Vec<DeleteArgument>,
    /// Read ROW and HEX from two-column TSV without a header (32 MiB, 4096 rows,
    /// subject to the shared operation budget). Mutually exclusive with --delete.
    #[arg(long, value_name = "PATH")]
    delete_file: Option<PathBuf>,
}

impl Requests {
    pub(super) fn load(
        self,
        budget: &mut ResourceBudget,
    ) -> Result<Vec<RowDelete>, Box<dyn Error>> {
        if let Some(path) = self.delete_file {
            let mut file = File::open(path)?;
            let length = tablecraft_io::seek_to_end(&mut file, budget, &|| false)?;
            if length > MAX_REQUEST_FILE_BYTES {
                return Err("Delete-request file exceeds the 32 MiB input limit".into());
            }
            budget.charge_allocation(length, 1)?;
            budget.charge(Resource::Steps, length)?;
            let length = usize::try_from(length)?;
            let mut input = Vec::new();
            input.try_reserve_exact(length)?;
            input.resize(length, 0);
            tablecraft_io::read_exact_at(&mut file, 0, &mut input, budget, &|| false)?;
            if tablecraft_io::seek_to_end(&mut file, budget, &|| false)? != length as u64 {
                return Err("Delete-request file length changed during capture".into());
            }
            return parse_tsv(&input, budget);
        }

        if self.deletes.len() > MAX_REQUEST_ROWS {
            return Err("Delete requests exceed the 4096-row input limit".into());
        }
        budget.charge_allocation(self.deletes.len() as u64, size_of::<RowDelete>() as u64)?;
        let mut result = Vec::new();
        result.try_reserve_exact(self.deletes.len())?;
        for argument in self.deletes {
            budget.charge(Resource::Rows, 1)?;
            budget.charge_allocation(argument.expected.len() as u64, 1)?;
            budget.charge(Resource::Steps, argument.expected.len() as u64)?;
            result.push(RowDelete::new(argument.row, argument.expected));
        }
        Ok(result)
    }
}

fn parse_tsv(input: &[u8], budget: &mut ResourceBudget) -> Result<Vec<RowDelete>, Box<dyn Error>> {
    if input.is_empty() {
        return Err("Delete-request file contains no requests".into());
    }
    // csv retains byte/string copies of its first record even without headers.
    // Prepay those two caches plus one reused record, including a hostile row
    // containing only delimiters. Each has at most input.len() + 1 field ends.
    budget.charge(Resource::Steps, input.len() as u64)?;
    budget.charge_allocation(READER_BUFFER_BYTES as u64, 1)?;
    let field_ends = input
        .len()
        .checked_add(1)
        .ok_or("Request input length overflow")?;
    for _ in 0..CSV_RETAINED_RECORDS {
        budget.charge_allocation(input.len() as u64, 1)?;
        budget.charge_allocation(field_ends as u64, size_of::<usize>() as u64)?;
    }
    budget.charge_allocation(MAX_REQUEST_ROWS as u64, size_of::<RowDelete>() as u64)?;
    let mut result = Vec::new();
    result.try_reserve_exact(MAX_REQUEST_ROWS)?;
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(false)
        .buffer_capacity(READER_BUFFER_BYTES)
        .from_reader(input);
    let mut record = StringRecord::with_capacity(input.len(), field_ends);
    while reader.read_record(&mut record)? {
        if record.len() != REQUEST_COLUMNS {
            return Err("Each delete-request record must contain exactly ROW and HEX".into());
        }
        if result.len() >= MAX_REQUEST_ROWS {
            return Err("Delete requests exceed the 4096-row input limit".into());
        }
        budget.charge(Resource::Rows, 1)?;
        budget.charge(Resource::Steps, record.as_slice().len() as u64)?;
        budget.charge_allocation((record[1].len() / HEX_DIGITS_PER_BYTE) as u64, 1)?;
        let argument = parse_parts(&record[0], &record[1]).map_err(|error| {
            format!(
                "Invalid delete-request record {}: {error}",
                result.len() + 1
            )
        })?;
        result.push(RowDelete::new(argument.row, argument.expected));
    }
    if result.is_empty() {
        return Err("Delete-request file contains no requests".into());
    }
    Ok(result)
}

fn parse_inline(text: &str) -> Result<DeleteArgument, String> {
    let (row, hex) = text
        .split_once(':')
        .ok_or("Expected ROW:HEX with a live-row ordinal and original record payload")?;
    parse_parts(row, hex)
}

fn parse_parts(row: &str, hex: &str) -> Result<DeleteArgument, String> {
    let row = row
        .parse::<usize>()
        .map_err(|_| "Row ordinal must be a nonnegative integer")?;
    if hex.is_empty()
        || !hex.len().is_multiple_of(HEX_DIGITS_PER_BYTE)
        || hex.len() / HEX_DIGITS_PER_BYTE > MAX_EXPECTED_ROW_BYTES
    {
        return Err(format!(
            "Expected a nonempty hexadecimal record of at most {MAX_EXPECTED_ROW_BYTES} bytes"
        ));
    }

    let mut expected = Vec::new();
    expected
        .try_reserve_exact(hex.len() / HEX_DIGITS_PER_BYTE)
        .map_err(|_| "Could not allocate expected record bytes")?;
    for pair in hex.as_bytes().as_chunks::<HEX_DIGITS_PER_BYTE>().0 {
        let high = digit(pair[0]).ok_or("Expected ASCII hexadecimal record bytes")?;
        let low = digit(pair[1]).ok_or("Expected ASCII hexadecimal record bytes")?;
        expected.push((high << 4) | low);
    }
    Ok(DeleteArgument { row, expected })
}

fn digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[path = "requests/tests.rs"]
mod tests;
