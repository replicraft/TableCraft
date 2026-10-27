//! Explicit read-only column-LVAL storage coverage, without a write preview.
use clap::Parser;
use std::{error::Error, fmt::Write as FmtWrite, fs::File, io::Write as IoWrite, path::PathBuf};
use tablecraft_ace::check_table_long_values;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ROWS: u64 = 100_000;
const MAX_STEPS: u64 = 100_000_000;

#[derive(Parser)]
#[command(about = "Check explicit stable/offline column long-value storage without writes")]
struct Args {
    database: PathBuf,
    table: String,
    /// Cumulative native allowance; storage/read ceilings remain unchanged.
    #[arg(long, default_value_t = MAX_BYTES, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    bytes: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut source = File::open(args.database)?;
    let mut budget = ResourceBudget::new(args.bytes, MAX_ROWS, MAX_STEPS);
    let checked = check_table_long_values(
        &mut source,
        CatalogScope::new(1),
        &args.table,
        &mut budget,
        &|| false,
    )?;
    let mut output = format!(
        "rows={}\nlong_columns={}\n",
        checked.rows(),
        checked.columns().len()
    );
    for column in checked.columns() {
        let values = column.values();
        writeln!(
            output,
            concat!(
                "column={} status={:?} owned_pages={} candidates={} owned_rows={} ",
                "distinct={} orphan={} shared_refs={} shared_pages={} ",
                "null={} inline={} single={} chained={}"
            ),
            column.column_number(),
            column.status(),
            column.owned_pages(),
            column.candidate_pages(),
            column.owned_rows(),
            column.distinct_chunks(),
            column.orphan_rows(),
            column.shared_references(),
            column.shared_owned_pages(),
            values.nulls(),
            values.inline(),
            values.single(),
            values.chained(),
        )?;
    }
    std::io::stdout().lock().write_all(output.as_bytes())?;
    Ok(())
}
