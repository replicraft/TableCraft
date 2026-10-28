//! Disposable ordinary-row INSERT candidate; never a native save.
#[path = "support/fixed_values.rs"]
mod fixed_values;
#[path = "preview_ace16_insert/input.rs"]
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
const REQUEST_VALUE_BYTES: usize = 1024;

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 ordinary-row INSERT candidate")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    table: String,
    /// Headerless quoted TSV: one KIND/DATA pair per column in native order;
    /// 16 MiB and 4096 complete rows, under one shared operation budget.
    #[arg(value_name = "TSV")]
    requests: PathBuf,
    /// Maximum encoded TSV DATA bytes per value; Binary counts hex digits.
    /// The 16 MiB file ceiling and shared operation budget still apply.
    #[arg(long, value_name = "BYTES", default_value_t = REQUEST_VALUE_BYTES, value_parser = input::value_bytes)]
    value_bytes: usize,
}

fn run(args: Args, budget: &mut ResourceBudget) -> Result<usize, Box<dyn Error>> {
    let requests = input::load(&args.requests, budget, args.value_bytes)?;
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), budget, &|| false)?;
    let candidate = image.preview_ace16_insert_rows(&args.table, &requests, budget, &|| false)?;

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
#[path = "preview_ace16_insert/tests.rs"]
mod tests;
