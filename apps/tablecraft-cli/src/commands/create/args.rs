use clap::Args;
use std::path::PathBuf;
use tablecraft_core::limits::ResourceBudget;

const WORK_BYTES: u64 = 256 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 100_000_000;

#[derive(Args)]
#[command(
    after_help = "Creates one fresh unencrypted ACE 2016 file with ordinary Int32 columns. Optional TSV starts with exactly the supplied column names in order. Every cell is a present decimal Int32 with optional minus sign; NULL, expressions and whitespace coercion are refused. Existing destinations are never overwritten. Required captured-directory durability must be supported. This does not enable ordinary editing of imported ACE files or certify native Access save compatibility."
)]
pub(crate) struct Arguments {
    /// Fresh destination path; an existing file, directory or symlink is refused.
    pub(super) path: PathBuf,
    /// Name of the one user table to construct.
    #[arg(long)]
    pub(super) table: String,
    /// Ordinary Int32 column name; repeat in the declared column order.
    #[arg(long = "column", required = true, action = clap::ArgAction::Append)]
    pub(super) columns: Vec<String>,
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
