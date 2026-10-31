use super::super::collation::UpdateCollation;
use clap::Args;
use std::path::PathBuf;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tablecraft_core::limits::ResourceBudget;

const WORK_BYTES: u64 = 256 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 100_000_000;

#[derive(Args)]
#[command(
    after_help = "Explicit offline in-place Jet4 Int32 UPDATE. Keep other applications and ordinary readers closed until this command or explicit recovery finishes. The journal directory must be fresh. Planning precedes journal creation; every commit error requires explicit recovery, never automatic SQL retry. This grants no ACE write or macro bypass. Supported filesystem lifecycle: macOS and Linux."
)]
pub(crate) struct UpdateArguments {
    /// One local single-assignment UPDATE, evaluated once against the original image.
    #[arg(allow_hyphen_values = true)]
    pub(super) sql: String,
    /// Existing offline Jet4 file, retained under one exclusive file lock.
    #[arg(long, value_name = "PATH")]
    pub(super) database: PathBuf,
    /// Fresh journal bundle directory; existing entries are never reused or replaced.
    #[arg(long, value_name = "DIRECTORY")]
    pub(super) journal: PathBuf,
    /// Explicit ordinal comparison for Text predicates; no ambient locale.
    #[arg(long, value_enum)]
    pub(super) collation: Option<UpdateCollation>,
    #[command(flatten)]
    pub(super) work: Work,
}

#[derive(Args)]
#[command(
    after_help = "Explicit offline recovery of the paired Jet4 journal bundle. No SQL is accepted or replayed. The persisted original typed request must reproduce both complete normal native images before any recovery write. Do not move or replace the target or journal bundle. Keep ordinary readers closed until recovery and namespace retirement finish. Supported filesystem lifecycle: macOS and Linux."
)]
pub(crate) struct RecoveryArguments {
    /// Original target object, retained under one exclusive file lock.
    #[arg(long, value_name = "PATH")]
    pub(super) database: PathBuf,
    /// Existing paired journal bundle retained after interruption or failure.
    #[arg(long, value_name = "DIRECTORY")]
    pub(super) journal: PathBuf,
    #[command(flatten)]
    pub(super) work: Work,
}

#[derive(Args)]
pub(super) struct Work {
    /// Cumulative capture, planning, intent, journal and lifecycle bytes.
    #[arg(long = "work-bytes", default_value_t = WORK_BYTES)]
    bytes: u64,
    /// Cumulative snapshot, evaluation and native row work.
    #[arg(long = "work-rows", default_value_t = WORK_ROWS)]
    rows: u64,
    /// Cumulative parsing, proof, journal and filesystem work.
    #[arg(long = "work-steps", default_value_t = WORK_STEPS)]
    steps: u64,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl Work {
    pub(super) fn budget(&self) -> ResourceBudget {
        ResourceBudget::new(self.bytes, self.rows, self.steps)
    }
}
