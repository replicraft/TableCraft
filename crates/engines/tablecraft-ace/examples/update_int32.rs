//! Engineering harness: write a fresh disposable copy, never modify the input.
use clap::Parser;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};
use tablecraft_ace::{Int32Update, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

#[derive(Parser)]
struct Args {
    source: PathBuf,
    output: PathBuf,
    table: String,
    column: String,
    row: usize,
    expected: i32,
    value: i32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut budget = ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000);
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), &mut budget, &|| false)?;
    let plan = image.plan_int32_updates(
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
    // This fresh-copy laboratory harness is not an in-place transaction. An I/O
    // failure may leave an incomplete output; the original is never written.
    output.write_all(plan.after_bytes())?;
    output.sync_all()?;
    println!("changed_rows={}", plan.changed_rows());
    Ok(())
}
