//! Read-only engineering checks with an explicit allocation/read allowance.
use clap::Parser;
use std::{fs::File, io::Write, path::PathBuf};
use tablecraft_ace::{
    IndexCheckStatus, RelationshipCheckStatus, RuleCheckStatus, check_table,
    check_table_constraints,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ROWS: u64 = 100_000;
const MAX_STEPS: u64 = 100_000_000;

#[derive(Parser)]
#[command(about = "Check an explicit stable/offline table without changing its source")]
struct Args {
    database: PathBuf,
    table: String,
    /// Cumulative native read/allocation allowance; format limits still apply.
    #[arg(long, default_value_t = MAX_BYTES, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    bytes: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut source = File::open(args.database)?;
    let mut budget = ResourceBudget::new(args.bytes, MAX_ROWS, MAX_STEPS);
    let structure = check_table(
        &mut source,
        CatalogScope::new(1),
        &args.table,
        &mut budget,
        &|| false,
    )?;
    let constraints = check_table_constraints(
        &mut source,
        CatalogScope::new(1),
        &args.table,
        &mut budget,
        &|| false,
    )?;
    if structure
        .indexes()
        .iter()
        .any(|index| index.status() != IndexCheckStatus::Checked)
        || constraints.table_validation() == RuleCheckStatus::Unsupported
        || constraints.fields().iter().any(|field| {
            [field.required(), field.empty_text(), field.validation()]
                .contains(&RuleCheckStatus::Unsupported)
        })
        || constraints
            .relationships()
            .iter()
            .any(|relationship| relationship.status() == RelationshipCheckStatus::Unsupported)
    {
        return Err("incomplete native check coverage".into());
    }
    // No output is published until both complete operations succeed. This is a
    // diagnostic profile, not a production-save or native Access qualification.
    let output = format!(
        "rows={}\nindexes_checked={}\nqualified_constraints=TRUE\n",
        structure.rows(),
        structure.indexes().len()
    );
    std::io::stdout().lock().write_all(output.as_bytes())?;
    Ok(())
}
