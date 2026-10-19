//! Exact-carrier engineering harness; no implicit coercion or source writes.
#[path = "support/fixed_values.rs"]
mod fixed_values;

use clap::Parser;
use fixed_values::{Kind, parse};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};
use tablecraft_ace::{FixedScalarUpdate, NativeImage};
use tablecraft_core::limits::ResourceBudget;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 fixed scalar candidate on a fresh copy")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    table: String,
    column: String,
    row: usize,
    #[arg(value_enum)]
    kind: Kind,
    #[arg(allow_negative_numbers = true)]
    expected: String,
    #[arg(allow_negative_numbers = true)]
    value: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let update = FixedScalarUpdate::new(
        args.row,
        parse(args.kind, &args.expected)?,
        parse(args.kind, &args.value)?,
    );
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(
        &mut source,
        tablecraft_model::CatalogScope::new(1),
        &mut budget,
        &|| false,
    )?;
    let preview = image.preview_ace16_fixed_updates(
        &args.table,
        &args.column,
        &[update],
        &mut budget,
        &|| false,
    )?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.output)?;
    output.write_all(preview.candidate_bytes())?;
    output.sync_all()?;
    println!("candidate_changed_rows={}", preview.changed_rows());
    eprintln!(
        "Engineering candidate: data-macro semantics and native Access qualification pending."
    );
    Ok(())
}
