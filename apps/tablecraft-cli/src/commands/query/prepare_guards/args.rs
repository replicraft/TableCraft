use clap::Args;
use std::path::PathBuf;

const WORK_BYTES: u64 = 256 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 100_000_000;

#[derive(Args)]
#[command(
    after_help = "Explicit read-only preparation of caller-selected source properties for one local all-explicit INSERT, one-column UPDATE or DELETE. Review exact KIND/NAME keys with inspect --extra. Caller order is engineering order, not native dispatch. Unselected XML never runs and required-macro coverage is not certified. No write candidate or output file is created. Uses Boolean-only If and provisional actual-value-change Updated. Unknown selected content fails closed; defaults, generation, cascades and ambient macro functions are unavailable."
)]
pub(crate) struct Arguments {
    /// One supported local SQL statement; supplied as one shell-quoted argument.
    #[arg(allow_hyphen_values = true)]
    pub(super) sql: String,
    /// Existing stable/offline source opened read-only; stored paths never open files.
    #[arg(long, value_name = "PATH")]
    pub(super) database: PathBuf,
    /// Explicit exact source properties, in engineering invocation order.
    #[arg(long = "guard-property", value_names = ["KIND", "NAME"], num_args = 2, required = true)]
    pub(super) guard_properties: Vec<String>,
    /// Include payload-free visited node/source-span metadata in the buffered report.
    #[arg(long)]
    pub(super) trace: bool,
    /// Cumulative syntax, source, guard and buffered report allowance.
    #[arg(long, default_value_t = WORK_BYTES)]
    pub(super) work_bytes: u64,
    /// Cumulative source/snapshot/selection/guard row allowance.
    #[arg(long, default_value_t = WORK_ROWS)]
    pub(super) work_rows: u64,
    /// Cumulative parsing, binding, evaluation and report work allowance.
    #[arg(long, default_value_t = WORK_STEPS)]
    pub(super) work_steps: u64,
}
