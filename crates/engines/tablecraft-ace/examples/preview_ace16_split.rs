//! Fresh, non-journalable Int32 leaf split candidate; never a native save.
#[path = "preview_ace16_slot/output.rs"]
mod output;

use clap::Parser;
use std::{error::Error, fs::File, path::PathBuf};
use tablecraft_ace::{Int32Update, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 Int32 leaf split candidate")]
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

fn run(args: Args, budget: &mut ResourceBudget) -> Result<usize, Box<dyn Error>> {
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), budget, &|| false)?;
    let preview = image.preview_ace16_split_int32_updates(
        &args.table,
        &args.column,
        &[Int32Update::new(args.row, args.expected, args.value)],
        budget,
        &|| false,
    )?;
    output::publish(&args.output, preview.candidate_bytes())?;
    Ok(preview.changed_rows())
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
#[allow(dead_code)]
#[path = "../tests/mutation/fixture.rs"]
mod fixture;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/mutation/index_fixture.rs"]
mod index_fixture;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/ace16_split_int32/model.rs"]
mod model;
#[cfg(test)]
use tablecraft_testkit::access as support;
#[cfg(test)]
#[path = "preview_ace16_split/tests.rs"]
mod tests;

#[cfg(test)]
use tablecraft_ace as engine;
