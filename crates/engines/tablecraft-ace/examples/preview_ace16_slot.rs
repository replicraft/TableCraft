//! Disposable Text/Binary candidate; never a qualified save.
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

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 Text/Binary candidate")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    table: String,
    column: String,
    /// Headerless ROW/EXPECTED_KIND/EXPECTED_DATA/KIND/DATA quoted TSV
    /// (16 MiB, 4096 rows; subject to the shared operation allowance).
    #[arg(value_name = "TSV")]
    requests: PathBuf,
    /// Use fitted records and same-page hidden-target forwarding, without page allocation.
    #[arg(long)]
    page_updates: bool,
}

fn run(args: Args, budget: &mut ResourceBudget) -> Result<usize, Box<dyn Error>> {
    // Capture and validate every assignment before opening the database or output.
    let requests = input::load(&args.requests, budget)?;
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), budget, &|| false)?;
    let candidate = if args.page_updates {
        image.preview_ace16_page_updates(&args.table, &args.column, &requests, budget, &|| false)?
    } else {
        image.preview_ace16_slot_updates(&args.table, &args.column, &requests, budget, &|| false)?
    };
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
#[path = "preview_ace16_slot/tests.rs"]
mod tests;
