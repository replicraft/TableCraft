use crate::commands::query::collation::UpdateCollation;
use clap::Args;
use std::path::PathBuf;

const WORK_BYTES: u64 = 256 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 100_000_000;

#[derive(Args)]
#[command(
    after_help = "Create one fresh experimental fitting ACE copy from a local one-column UPDATE and explicitly selected BeforeChange properties. Caller property order is engineering order, not native dispatch or required-program coverage. Boolean-only If and provisional actual-value-change Updated remain explicit limits. Current physical rows are ordinary Int32/NULL; defaults, generation, growth, cascades and ambient macro functions are unavailable. Unknown selected content or RaiseError returns no candidate. Original macro bytes remain unchanged. This grants no normal save, journal or required-macro permission. --durable-output requires the existing captured-directory publication barriers without fallback. Encrypted/deployment sources refuse."
)]
pub(crate) struct Arguments {
    /// Local single-assignment UPDATE, evaluated against the original snapshot.
    #[arg(allow_hyphen_values = true)]
    pub(super) sql: String,
    /// Existing stable/offline raw source; stored content never authorizes opens.
    #[arg(long, value_name = "PATH")]
    pub(super) database: PathBuf,
    /// Fresh output; files, source aliases and symlinks are never replaced.
    #[arg(long, value_name = "PATH")]
    pub(super) output: PathBuf,
    /// Exact caller-selected properties in explicit engineering invocation order.
    #[arg(long = "guard-property", value_names = ["KIND", "NAME"], num_args = 2, required = true)]
    pub(super) guard_properties: Vec<String>,
    /// Include payload-free control/assignment positions, never XML or row values.
    #[arg(long)]
    pub(super) trace: bool,
    /// Require supported file and captured-directory publication barriers.
    #[arg(long)]
    pub(super) durable_output: bool,
    /// Explicit ordinal policy for SQL Text predicates; no ambient locale.
    #[arg(long, value_enum)]
    pub(super) collation: Option<UpdateCollation>,
    /// Cumulative syntax, source, candidate, report and publication bytes.
    #[arg(long, default_value_t = WORK_BYTES)]
    pub(super) work_bytes: u64,
    /// Cumulative snapshot, selection and native row work.
    #[arg(long, default_value_t = WORK_ROWS)]
    pub(super) work_rows: u64,
    /// Cumulative parsing, binding, evaluation, report and publication work.
    #[arg(long, default_value_t = WORK_STEPS)]
    pub(super) work_steps: u64,
}
