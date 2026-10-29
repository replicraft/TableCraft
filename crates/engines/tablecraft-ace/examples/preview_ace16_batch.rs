//! Fixed-cell engineering batches on fresh copies; cascades and growth are opt-in.
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
    /// Allow one table's Int32 indexes to grow in this engineering candidate.
    #[arg(long, conflicts_with = "cascade_updates")]
    index_growth: bool,
    #[arg(long="set",required=true,num_args=SPEC_FIELDS,action=clap::ArgAction::Append,allow_hyphen_values=true,value_names=["TABLE","COLUMN","ROW","KIND","EXPECTED","VALUE"])]
    cells: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Fitted,
    Cascading,
    IndexGrowth,
}

impl Args {
    fn mode(&self) -> Mode {
        if self.index_growth {
            Mode::IndexGrowth
        } else if self.cascade_updates {
            Mode::Cascading
        } else {
            Mode::Fitted
        }
    }
}

struct Group<'a> {
    table: &'a str,
    column: &'a str,
    updates: Vec<FixedScalarUpdate>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mode = args.mode();
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
    let preview = match mode {
        Mode::IndexGrowth => {
            image.preview_ace16_split_batch_updates(&groups, &mut budget, &|| false)?
        }
        Mode::Cascading => {
            image.preview_ace16_cascading_updates(&groups, &mut budget, &|| false)?
        }
        Mode::Fitted => image.preview_ace16_batch_updates(&groups, &mut budget, &|| false)?,
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

#[cfg(test)]
#[path = "preview_ace16_batch/tests.rs"]
mod tests;
