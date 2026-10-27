//! Fresh ordinary Memo/OLE candidate; no native save or journal conversion.
#[path = "preview_ace16_slot/input.rs"]
mod input;
#[path = "preview_ace16_slot/output.rs"]
mod output;

use clap::Parser;
use std::{error::Error, fs::File, path::PathBuf};
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;
const REQUEST_VALUE_BYTES: usize = 64 * 1024;

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 ordinary Memo/OLE candidate")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    table: String,
    column: String,
    /// Headerless ROW/EXPECTED_KIND/EXPECTED_DATA/KIND/DATA quoted TSV.
    /// Text is UTF-8; Binary is hex. Each encoded value is at most 64 KiB.
    /// The file is at most 16 MiB/4096 requests, subject to the shared allowance.
    requests: PathBuf,
}

fn run(args: Args, budget: &mut ResourceBudget) -> Result<usize, Box<dyn Error>> {
    let requests = input::load(&args.requests, budget, REQUEST_VALUE_BYTES)?;
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), budget, &|| false)?;
    let candidate = image.preview_ace16_long_value_updates(
        &args.table,
        &args.column,
        &requests,
        budget,
        &|| false,
    )?;
    output::publish(&args.output, candidate.candidate_bytes())?;
    Ok(candidate.changed_rows())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    let changed = run(args, &mut budget)?;
    println!("candidate_changed_rows={changed}");
    eprintln!(
        "Engineering candidate: data-macro semantics and native Access qualification pending."
    );
    Ok(())
}

#[cfg(test)]
#[path = "preview_ace16_long_value/tests.rs"]
mod tests;
