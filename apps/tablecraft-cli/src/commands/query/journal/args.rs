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
        "Explicit offline in-place Int32 UPDATE for Jet4 or the narrow positive-program ACE BeforeChange profile. ",
        "Keep other applications and ordinary readers closed until this command or explicit recovery finishes. ",
        "The journal directory must be fresh. Planning precedes journal creation; every commit error requires explicit recovery, never automatic SQL retry. ",
        "ACE requires unencoded ACE2016/2019 subversion zero, complete source-bound pure BeforeChange coverage and no relationships. ",
        "Tables containing only ordinary updatable Int32 fields support checked non-NULL indexes with compressed prefixes, retained interior nodes, composites, aliases and final unique-key swaps. ",
        "Mixed schemas may preserve admitted scalar/rich siblings and unchanged indexes while updating unindexed ordinary Int32 fields. Ordinary Int32 AutoNumber operands are readable, never writable. ",
        "Changed targets remain non-NULL and outside calculation/collection dependencies. Fitting retains page identities, topology and leaf entry counts. ",
        "Ignore-NULL index rewriting, splits and map/storage growth refuse. Supplied equal assignments refuse with EventTiming; unknown required content cannot be skipped. ",
        "This does not enable general ACE saves or certify macro absence. Supported filesystem lifecycle: macOS and Linux.",
    )
)]
pub(crate) struct UpdateArguments {
    /// One local single-assignment UPDATE, evaluated once against the original image.
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
    after_help = "Explicit offline recovery of a paired Jet4 or admitted ACE BeforeChange journal bundle. No SQL is accepted or replayed. The persisted original typed request must reproduce both complete normal native images before any recovery write; ACE recovery rederives its bounded pure required programs from BEFORE. Do not move or replace the target or journal bundle. Keep ordinary readers closed until recovery and namespace retirement finish. Supported filesystem lifecycle: macOS and Linux."
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
