//! Explicit engineering candidate: fresh offline copies, pending native qualification.
use clap::Parser;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};
use tablecraft_ace::{Int32Update, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 physical write candidate for native comparison")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    table: String,
    column: String,
    row: usize,
    #[arg(allow_negative_numbers = true)]
    expected: i32,
    #[arg(allow_negative_numbers = true)]
    value: i32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), &mut budget, &|| false)?;
    let preview = image.preview_ace16_int32_updates(
        &args.table,
        &args.column,
        &[Int32Update::new(args.row, args.expected, args.value)],
        &mut budget,
        &|| false,
    )?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.output)?;
    // A failure can leave an incomplete disposable copy. Never overwrite input
    // or invoke a journal/commit path for an unqualified preview.
    output.write_all(preview.candidate_bytes())?;
    output.sync_all()?;
    println!("candidate_changed_rows={}", preview.changed_rows());
    eprintln!(
        "Engineering candidate: data-macro semantics and native Access qualification pending."
    );
    Ok(())
}
