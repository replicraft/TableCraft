//! Fixed-cell engineering batches on fresh disposable copies; cascades are opt-in.
#[path = "support/fixed_values.rs"]
mod fixed_values;

use clap::{Parser, ValueEnum};
use fixed_values::{Kind, parse};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};
use tablecraft_ace::{FixedColumnUpdates, FixedScalarUpdate, NativeImage};
use tablecraft_core::limits::ResourceBudget;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;
const SPEC_FIELDS: usize = 6;

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 fixed-field batch candidate on a fresh copy")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    /// Expand declared update cascades using original relationship matches.
    #[arg(long)]
    cascade_updates: bool,
    #[arg(long="set",required=true,num_args=SPEC_FIELDS,action=clap::ArgAction::Append,allow_negative_numbers=true,value_names=["TABLE","COLUMN","ROW","KIND","EXPECTED","VALUE"])]
    cells: Vec<String>,
}
struct Group<'a> {
    table: &'a str,
    column: &'a str,
    updates: Vec<FixedScalarUpdate>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut owned: Vec<Group<'_>> = Vec::new();
    let (cells, remainder) = args.cells.as_chunks::<SPEC_FIELDS>();
    if !remainder.is_empty() || cells.len() > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS {
        return Err("invalid batch shape or assignment limit exceeded".into());
    }
    for fields in cells {
        let kind = Kind::from_str(&fields[3], false)?;
        let update = FixedScalarUpdate::new(
            fields[2].parse()?,
            parse(kind, &fields[4])?,
            parse(kind, &fields[5])?,
        );
        if let Some(group) = owned.iter_mut().find(|group| {
            group.table.eq_ignore_ascii_case(&fields[0])
                && group.column.eq_ignore_ascii_case(&fields[1])
        }) {
            group.updates.push(update);
        } else {
            owned.push(Group {
                table: &fields[0],
                column: &fields[1],
                updates: vec![update],
            });
        }
    }
    let groups = owned
        .iter()
        .map(|group| FixedColumnUpdates::new(group.table, group.column, &group.updates))
        .collect::<Vec<_>>();
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    let image = NativeImage::capture(
        &mut File::open(&args.source)?,
        tablecraft_model::CatalogScope::new(1),
        &mut budget,
        &|| false,
    )?;
    let preview = if args.cascade_updates {
        image.preview_ace16_cascading_updates(&groups, &mut budget, &|| false)?
    } else {
        image.preview_ace16_batch_updates(&groups, &mut budget, &|| false)?
    };
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.output)?;
    output.write_all(preview.candidate_bytes())?;
    output.sync_all()?;
    println!("candidate_changed_rows={}", preview.changed_rows());
    eprintln!(
        "Engineering candidate: delete actions, data-macro semantics and native Access qualification pending."
    );
    Ok(())
}
