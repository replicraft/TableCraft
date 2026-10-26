//! Create a disposable physical deletion candidate; never a qualified save.
use clap::Parser;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};
use tablecraft_ace::{NativeImage, RowDelete};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;
const MAX_EXPECTED_ROW_BYTES: usize = 4096;
const HEX_DIGITS_PER_BYTE: usize = 2;

#[derive(Clone)]
struct DeleteArgument {
    row: usize,
    expected: Vec<u8>,
}

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 physical deletion candidate for comparison")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    table: String,
    /// Zero-based live-row ordinal and exact original record payload in hexadecimal.
    /// Includes the record column-count header and null mask, excluding page/slot words.
    #[arg(long = "delete", value_name = "ROW:HEX", required = true, value_parser = parse_delete)]
    deletes: Vec<DeleteArgument>,
}

fn parse_delete(text: &str) -> Result<DeleteArgument, String> {
    let (row, hex) = text
        .split_once(':')
        .ok_or("Expected ROW:HEX with a live-row ordinal and original record payload")?;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let deletes: Vec<_> = args
        .deletes
        .into_iter()
        .map(|request| RowDelete::new(request.row, request.expected))
        .collect();
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), &mut budget, &|| false)?;
    let candidate = image.preview_ace16_deletes(&args.table, &deletes, &mut budget, &|| false)?;

    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.output)?;
    // Output failure may leave an incomplete disposable copy. The source and
    // qualified journal/apply services are never written through this example.
    output.write_all(candidate.candidate_bytes())?;
    output.sync_all()?;
    println!("candidate_deleted_rows={}", candidate.deleted_rows());
    eprintln!(
        "Engineering candidate: data-macro semantics and native Access qualification pending."
    );
    Ok(())
}
