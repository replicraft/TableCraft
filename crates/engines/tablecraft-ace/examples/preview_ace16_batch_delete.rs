//! Disposable multi-table deletion candidates; never a qualified save.
#[path = "preview_ace16_batch_delete/input.rs"]
mod input;
#[path = "preview_ace16_delete/requests.rs"]
mod requests;

use clap::Parser;
use std::{
    error::Error,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tablecraft_ace::{NativeImage, TableRowDeletes};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;

#[derive(Parser)]
#[command(about = "Create an unqualified multi-table ACE 16 deletion candidate")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    /// Expand qualified acyclic cross-table Cascade actions from the original seeds.
    #[arg(long)]
    cascade: bool,
    /// Exact table selector and headerless ROW/HEX TSV, repeatable for distinct tables.
    #[arg(long = "group", required = true, num_args = 2, action = clap::ArgAction::Append,
        value_names = ["TABLE", "DELETE_TSV_PATH"])]
    groups: Vec<OsString>,
}

fn run(args: Args, budget: &mut ResourceBudget) -> Result<usize, Box<dyn Error>> {
    // Capture every external request before opening the database or any output.
    let groups = input::load(args.groups, budget)?;
    budget.charge_allocation(groups.len() as u64, size_of::<TableRowDeletes<'_>>() as u64)?;
    let mut borrowed = Vec::new();
    borrowed.try_reserve_exact(groups.len())?;
    for group in &groups {
        borrowed.push(TableRowDeletes::new(&group.table, &group.deletes));
    }
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), budget, &|| false)?;
    let candidate = if args.cascade {
        image.preview_ace16_cascading_deletes(&borrowed, budget, &|| false)?
    } else {
        image.preview_ace16_batch_deletes(&borrowed, budget, &|| false)?
    };
    publish(&args.output, candidate.candidate_bytes())?;
    Ok(candidate.deleted_rows())
}

fn publish(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut output = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = output.write_all(bytes).and_then(|()| output.sync_all()) {
        drop(output);
        if let Err(cleanup) = fs::remove_file(path) {
            return Err(format!(
                "Candidate write failed ({error}); incomplete-output cleanup failed ({cleanup})"
            )
            .into());
        }
        return Err(error.into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    let deleted = run(args, &mut budget)?;
    println!("candidate_deleted_rows={deleted}");
    eprintln!(
        "Engineering candidate: data-macro semantics and native Access qualification pending."
    );
    Ok(())
}

#[cfg(test)]
#[path = "preview_ace16_batch_delete/tests.rs"]
mod tests;
