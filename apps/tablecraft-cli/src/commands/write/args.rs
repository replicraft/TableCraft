//! Explicit engineering operations and host work limits.
use super::input;
use clap::{Args, Subcommand};
use std::path::PathBuf;
use tablecraft_core::limits::ResourceBudget;

const WRITE_BYTES: u64 = 256 * 1024 * 1024;
const WRITE_ROWS: u64 = 100_000;
const WRITE_STEPS: u64 = 100_000_000;

#[derive(Args)]
pub(super) struct Paths {
    /// Stable/offline raw MDB/ACCDB source; never written or replaced.
    pub source: PathBuf,
    /// Fresh output path; existing files and symlinks are never overwritten.
    pub output: PathBuf,
    #[command(flatten)]
    pub limits: Limits,
}

#[derive(Args)]
pub(super) struct Limits {
    /// Cumulative request/native/output byte allowance; image caps still apply.
    #[arg(long, default_value_t = WRITE_BYTES)]
    work_bytes: u64,
    /// Cumulative request/native row allowance, not a table-format maximum.
    #[arg(long, default_value_t = WRITE_ROWS)]
    work_rows: u64,
    /// Cumulative parsing/native/publication work allowance.
    #[arg(long, default_value_t = WRITE_STEPS)]
    work_steps: u64,
}

impl Limits {
    pub(super) fn budget(&self) -> ResourceBudget {
        ResourceBudget::new(self.work_bytes, self.work_rows, self.work_steps)
    }
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Make an experimental expected-value UPDATE copy; required macros do not run.
    #[command(name = "preview-update")]
    Update(Update),
    /// Make an experimental complete-row INSERT copy from quoted typed TSV.
    #[command(name = "preview-insert")]
    Insert(Insert),
    /// Make an experimental DELETE copy using exact original record bytes.
    #[command(name = "preview-delete")]
    Delete(Delete),
}

#[derive(Args)]
pub(crate) struct Update {
    #[command(flatten)]
    pub(super) paths: Paths,
    /// One exact original-row assignment. NULL is `null`; `value:null` is literal
    /// text "null"; prefix any literal starting `value:` with another `value:`.
    #[arg(long = "set", required = true, num_args = super::updates::SET_FIELDS, action = clap::ArgAction::Append, allow_hyphen_values = true, value_names = ["TABLE", "COLUMN", "ROW", "KIND", "EXPECTED", "VALUE"])]
    pub(super) cells: Vec<String>,
    /// Short Text/Binary DATA placement in one table, optionally with fixed siblings.
    #[arg(long, conflicts_with_all = ["long_value", "cascade_updates"])]
    pub(super) variable: bool,
    /// Ordinary Memo/OLE replacement in one table/column using existing LVAL storage.
    #[arg(long, conflicts_with_all = ["variable", "index_growth", "cascade_updates"])]
    pub(super) long_value: bool,
    /// Permit fixed or single-variable INDEX growth; mixed variable growth refuses.
    #[arg(long, conflicts_with = "cascade_updates")]
    pub(super) index_growth: bool,
    /// Expand qualified update cascades from original relationship associations.
    #[arg(long)]
    pub(super) cascade_updates: bool,
    /// Maximum bytes per expected/replacement argument, including escape prefixes.
    #[arg(long, default_value_t = input::DEFAULT_VALUE_BYTES, value_parser = input::value_limit)]
    pub(super) value_bytes: usize,
}

#[derive(Args)]
pub(crate) struct Insert {
    #[command(flatten)]
    pub(super) paths: Paths,
    /// Exact local table name.
    pub(super) table: String,
    /// Headerless quoted TSV: KIND/DATA pairs in native field order. `null` KIND
    /// requires empty DATA. Text DATA "null" is always literal text.
    pub(super) requests: PathBuf,
    /// Explicit increment policy and original LAST-issued counter precondition.
    /// TSV contains only companions excluding the generated primary column.
    #[arg(long, allow_hyphen_values = true, conflicts_with = "incrementing")]
    pub(super) incrementing_last: Option<i32>,
    /// Explicitly select increment-by-one behavior using LAST from this captured
    /// image. Stored Increment/Random mode is not detected. TSV supplies companions.
    #[arg(long, conflicts_with = "incrementing_last")]
    pub(super) incrementing: bool,
    /// Maximum TSV DATA bytes per value; binary counts encoded hexadecimal digits.
    #[arg(long, default_value_t = input::DEFAULT_VALUE_BYTES, value_parser = input::value_limit)]
    pub(super) value_bytes: usize,
}

#[derive(Args)]
pub(crate) struct Delete {
    #[command(flatten)]
    pub(super) paths: Paths,
    /// Exact local table name.
    pub(super) table: String,
    #[command(flatten)]
    pub(super) requests: Deletes,
    /// Expand qualified declared CASCADE DELETE and SET NULL actions.
    #[arg(long)]
    pub(super) cascade_deletes: bool,
}

#[derive(Args)]
#[group(id = "delete_requests", required = true, multiple = false)]
pub(super) struct Deletes {
    /// Original zero-based live-row ordinal; the engine captures exact expected
    /// bytes from the same immutable source before preparing the DELETE.
    #[arg(long = "row", value_name = "ORDINAL")]
    pub ordinals: Vec<usize>,
    /// Original zero-based live-row ordinal and exact original record HEX,
    /// including column-count header/null mask but excluding page/slot words.
    #[arg(long = "delete", value_name = "ROW:HEX")]
    pub inline: Vec<String>,
    /// Headerless quoted two-column ROW/HEX TSV (16 MiB, at most 4096 requests).
    #[arg(long, value_name = "TSV")]
    pub delete_file: Option<PathBuf>,
}
