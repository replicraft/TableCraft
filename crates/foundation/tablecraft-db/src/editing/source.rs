use crate::{CellUpdate, Snapshot};
use tablecraft_core::limits::ResourceBudget;

/// Candidate update shape, not authorization for a particular table/field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateProfile {
    /// No implemented update planner for this source/profile.
    Unavailable,
    /// Existing non-NULL Int32 replacements; backend-specific gates still apply.
    ExistingInt32,
    /// Stored scalar carriers and nullable NULL. Int32 targets retain the
    /// existing checked integral assignment law; other types remain exact.
    /// Backend rules still decide whether an actual field can be changed.
    StoredScalar,
}

/// One frozen table plus a consuming, all-or-nothing edit planner. Planning must
/// perform no durable writes. The concrete plan owns persistence/recovery policy;
/// successful planning does not establish a committed transaction.
pub trait EditableTable: Sized {
    /// Backend-owned sanitized error, retaining typed causes.
    type Error: std::error::Error;
    /// Backend-owned validated candidate, not an implicit commit.
    type Plan;
    /// Frozen schema/rows whose handles may be used in this plan.
    fn snapshot(&self) -> &Snapshot;
    /// Candidate carrier profile; unknown constraints must still block planning.
    fn update_profile(&self) -> UpdateProfile;
    /// Plan one column's replacements, rejecting stale/repeated handles, wrong
    /// carriers and unavailable behavior. Failure publishes no partial plan.
    fn plan_updates(
        self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error>;
}
