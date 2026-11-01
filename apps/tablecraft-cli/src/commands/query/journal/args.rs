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
    after_help = concat!(
        "Offline UPDATE for qualified Jet4 and ACE16 profiles. Jet4 supports one Int32 SET target. ",
        "Keep other applications and ordinary readers closed until this command or explicit recovery finishes. ",
        "The journal directory must be fresh. Planning precedes journal creation; commit failures require explicit recovery rather than automatic SQL retry. ",
        "ACE requires an unencoded ACE2016/2019 subversion-zero file without relationships. Supported fixed targets are Boolean, Int32, Int64, Currency and legacy DATE; numeric NULL transitions preserve inactive bytes and Boolean is nonnullable. ",
        "Qualified ordinary tables also support unindexed Short Text/Binary batches and optional fixed siblings within existing record capacity. Raw noops and empty selections remain distinct from NULL and present-empty values. ",
        "Qualified ACE16 tables can also grow records using space in the same page. Planning selects the mode before staging; page allocation and file growth remain unsupported. ",
        "The ordinary fixed route supports qualified retained Int32 indexes, including fitting NULL-key transitions and atomic unique-key swaps. AutoNumber fields remain read-only. ",
        "All SET expressions read one original snapshot. Required programs need supported native coverage; macro assignments retain the Int32/NULL language. Unsupported targets, properties, required programs and allocation changes refuse, including empty operations. ",
        "This command does not enable general ACE saves or establish native Access conformance. Supported filesystem lifecycle: macOS and Linux.",
    )
)]
pub(crate) struct UpdateArguments {
    /// One local UPDATE; all SET expressions read the same original snapshot.
    #[arg(allow_hyphen_values = true)]
    pub(super) sql: String,
    /// Existing offline file in an admitted native profile, retained under one exclusive lock.
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
    after_help = "Explicit offline recovery of a paired Jet4 or admitted ACE journal bundle. No SQL is accepted or replayed. The persisted original typed request must reproduce both complete normal native images before any recovery write; ACE recovery rederives its bounded pure required programs or closed data-only admission from BEFORE. Do not move or replace the target or journal bundle. Keep ordinary readers closed until recovery and namespace retirement finish. Supported filesystem lifecycle: macOS and Linux."
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
#[command(
    after_help = "Confirm the current original/replacement image of an intact retired native journal bundle, then acknowledge its captured-parent barrier and clean up. No SQL, rollback or recovery replay is performed. Missing records, partial garbage, active bundles and mismatched current images refuse without fallback. The image report does not establish historical commit/rollback events. Keep other applications and ordinary readers closed until completion. Supported filesystem lifecycle: macOS and Linux."
)]
pub(crate) struct FinalizeArguments {
    /// Original target object, retained under one exclusive file lock.
    #[arg(long, value_name = "PATH")]
    pub(super) database: PathBuf,
    /// Intact nonce-named retired bundle, selected explicitly after interruption.
    #[arg(long, value_name = "RETIRED_DIRECTORY")]
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
