//! Create a disposable physical deletion candidate; never a qualified save.
#[path = "preview_ace16_delete/requests.rs"]
mod requests;

use clap::Parser;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 physical deletion candidate for comparison")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    table: String,
    #[command(flatten)]
    requests: requests::Requests,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    let deletes = args.requests.load(&mut budget)?;
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
