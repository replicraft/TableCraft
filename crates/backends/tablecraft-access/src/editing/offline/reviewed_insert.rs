//! Fresh sealed INSERT qualification retains original presence and captured inputs.
use super::{Cause, Failure, OfflineWriteSession, Phase, SessionState, StagedInsert};
use crate::editing::journal::NativeJournalInsertPlan;
use tablecraft_core::limits::ResourceBudget;

impl OfflineWriteSession {
    /// Consume one source-bound reviewed INSERT without reevaluating SQL or
    /// resampling host clocks. Fresh native reconstruction rechecks stored
    /// defaults from the same source and retained clock input. The original
    /// selected native family is retained.
    /// Capture/rebuild spend `native`; immutable source/image proof spends `proof`.
    /// Failure retains this same session and creates no journal namespace.
    pub fn stage_reviewed_insert_with_budgets(
        mut self,
        reviewed: NativeJournalInsertPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedInsert, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_reviewed_insert").entered();
        let result = (|| {
            self.require_ready()?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = reviewed
                .capture_original(
                    slot.target.as_mut().ok_or(Cause::InvalidState)?,
                    slot.scope,
                    native,
                    cancelled,
                )
                .map_err(Cause::Native)?;
            slot.state = SessionState::Ready;
            super::table::require_original(
                image.as_bytes(),
                reviewed.before_bytes(),
                proof,
                cancelled,
            )?;
            reviewed
                .requalify_with_budgets(image, native, proof, cancelled)
                .map_err(Cause::Qualification)
        })();
        match result {
            Ok(plan) => Ok(StagedInsert {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
