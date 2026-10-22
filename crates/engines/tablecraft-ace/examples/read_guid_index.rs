//! Read-only engineering host for exact GUID-index comparison on stable copies.
use clap::{ArgGroup, Parser};
use std::{fs::File, io::Write, path::PathBuf};
use tablecraft_ace::{GuidIndexSelection, read_table_guid_index};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Value},
};
use tablecraft_model::CatalogScope;
use uuid::Uuid;

const BYTE_BUDGET: u64 = 128 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;
const MAX_ID_LINE_BYTES: usize = i32::MIN.unsigned_abs().ilog10() as usize + 2;

#[derive(Parser)]
#[command(about = "Read Int32 row IDs through a GUID index on an explicit stable/offline copy")]
#[command(group(ArgGroup::new("selection").required(true).args(["all", "null", "equal"])))]
struct Args {
    source: PathBuf,
    table: String,
    index: String,
    #[arg(long)]
    all: bool,
    #[arg(long)]
    null: bool,
    #[arg(long)]
    equal: Option<Uuid>,
    #[arg(long, default_value = "ID")]
    id_column: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let selection = if args.all {
        GuidIndexSelection::All
    } else if args.null {
        GuidIndexSelection::Null
    } else {
        GuidIndexSelection::Equal(Guid::from_bytes(
            args.equal.ok_or("missing selection")?.into_bytes(),
        ))
    };
    let mut source = File::open(args.source)?;
    let snapshot = read_table_guid_index(
        &mut source,
        CatalogScope::new(1),
        &args.table,
        &args.index,
        selection,
        &mut ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET),
        &|| false,
    )?;
    let column = snapshot
        .columns()
        .iter()
        .position(|column| column.name().eq_ignore_ascii_case(&args.id_column))
        .ok_or("missing ID column")?;
    let mut output = Vec::new();
    output.try_reserve_exact(
        snapshot
            .rows()
            .len()
            .checked_mul(MAX_ID_LINE_BYTES)
            .ok_or("output limit")?,
    )?;
    for row in snapshot.rows() {
        let Value::Int32(value) = row[column] else {
            return Err("expected Int32 row IDs".into());
        };
        writeln!(&mut output, "{value}")?;
    }
    std::io::stdout().lock().write_all(&output)?;
    Ok(())
}
