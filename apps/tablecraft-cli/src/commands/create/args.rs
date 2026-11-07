use clap::Args;
use std::path::PathBuf;
use tablecraft_core::limits::ResourceBudget;

const WORK_BYTES: u64 = 256 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 100_000_000;

#[derive(Args)]
#[command(group(
    clap::ArgGroup::new("creation_schema")
        .args(["blank", "columns", "fields"])
        .required(true)
        .multiple(false)
))]
#[command(
    after_help = "Creates one fresh unencrypted ACE 2016 file. Explicit --blank creates only system metadata, with no user tables or template. Otherwise --table and exactly one column/field schema are required. --column retains present decimal Int32 cells with optional minus sign and refuses NULL or whitespace coercion. Exclusive --field NAME DECLARATION supports explicit fixed carriers, text:UTF16_UNITS, binary:BYTES and decimal:PRECISION:SCALE; typed cells use null/value: escapes. Text and Binary limits are 1..255, Decimal scale is exact. Optional quoted TSV starts with exactly the supplied names in order. No expressions or inferred declarations. --primary-key COLUMN INDEX_NAME explicitly names one Int32/Int64/Currency primary key. Optional --incrementing COLUMN explicitly declares an empty LONG Increment primary; the current profile requires two Int32 columns, the generated column first, no initial rows or defaults. No Random generation or reseeding is inferred. Existing destinations are never overwritten. Required captured-directory durability must be supported. This does not enable ordinary editing of imported ACE files or certify native Access save compatibility."
)]
pub(crate) struct Arguments {
    /// Fresh destination path; an existing file, directory or symlink is refused.
    pub(super) path: PathBuf,
    /// Create a system-only database with no user tables, columns or records.
    #[arg(long, conflicts_with_all = ["table", "columns", "fields", "data", "primary_key", "incrementing"])]
    pub(super) blank: bool,
    /// Name of the one user table to construct; required unless --blank is used.
    #[arg(long, required_unless_present = "blank")]
    pub(super) table: Option<String>,
    /// Ordinary Int32 column name; repeat in the declared column order.
    #[arg(long = "column", action = clap::ArgAction::Append)]
    pub(super) columns: Vec<String>,
    /// Explicit typed column; repeat NAME DECLARATION in the written field order.
    /// Fixed kinds use the exact write carrier spelling; variables require text:N,
    /// binary:N or decimal:P:S. NULL is null; value:null is literal Text "null".
    #[arg(long = "field", num_args = 2, action = clap::ArgAction::Append, value_names = ["NAME", "DECLARATION"])]
    pub(super) fields: Vec<String>,
    /// One primary key: declared column selector followed by index name.
    /// Supports Int32, Int64 and Currency; never infers AutoNumber or defaults.
    #[arg(long, num_args = 2, action = clap::ArgAction::Set, value_names = ["COLUMN", "INDEX_NAME"])]
    pub(super) primary_key: Vec<String>,
    /// Explicit LONG Increment declaration; requires the same primary column.
    /// Empty tables only; initial IDs are never generated or coerced by create.
    #[arg(long, value_name = "COLUMN", requires = "primary_key")]
    pub(super) incrementing: Option<String>,
    /// Bounded quoted TSV with an exact column header and zero through 64 rows.
    #[arg(long, value_name = "TSV")]
    pub(super) data: Option<PathBuf>,
    /// Cumulative input, construction and publication byte allowance.
    #[arg(long, default_value_t = WORK_BYTES)]
    work_bytes: u64,
    /// Cumulative input and construction row allowance.
    #[arg(long, default_value_t = WORK_ROWS)]
    work_rows: u64,
    /// Cumulative parsing, native construction and publication work allowance.
    #[arg(long, default_value_t = WORK_STEPS)]
    work_steps: u64,
}

impl Arguments {
    pub(super) fn budget(&self) -> ResourceBudget {
        ResourceBudget::new(self.work_bytes, self.work_rows, self.work_steps)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Cli, commands::Command};
    use clap::Parser;

    fn parse(options: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(
            ["tablecraft", "create", "new.accdb"]
                .into_iter()
                .chain(options.iter().copied()),
        )
    }

    #[test]
    fn explicit_blank_conflicts_with_every_table_input_without_loosening_legacy_requirements() {
        let parsed = parse(&["--blank"]).unwrap();
        let Some(Command::Create(arguments)) = parsed.command else {
            panic!("expected create command");
        };
        assert!(arguments.blank);
        assert!(arguments.table.is_none());
        assert!(arguments.columns.is_empty());
        assert!(arguments.fields.is_empty());

        for options in [
            vec!["--table", "Items"],
            vec!["--column", "Id"],
            vec!["--field", "Id", "int32"],
            vec!["--data", "must-not-be-opened.tsv"],
            vec!["--primary-key", "Id", "PrimaryKey"],
            vec!["--incrementing", "Id"],
        ] {
            let mut blank = vec!["--blank"];
            blank.extend(options);
            assert!(parse(&blank).is_err(), "conflicting arguments: {blank:?}");
        }
        for options in [
            vec![],
            vec!["--table", "Items"],
            vec!["--column", "Id"],
            vec!["--field", "Id", "int32"],
            vec![
                "--table", "Items", "--column", "Id", "--field", "Id", "int32",
            ],
        ] {
            assert!(parse(&options).is_err(), "legacy refusal: {options:?}");
        }
        assert!(parse(&["--table", "Items", "--column", "Id"]).is_ok());
        assert!(parse(&["--table", "Items", "--field", "Id", "int32"]).is_ok());
    }
}
