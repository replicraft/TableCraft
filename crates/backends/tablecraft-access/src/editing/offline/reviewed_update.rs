//! Fresh native qualification of a lease-free reviewed UPDATE, without SQL replay.
use super::{Cause, Failure, OfflineWriteSession, Phase, SessionState, StagedUpdate};
use tablecraft_ace::{NativeImage, RecoverableNativeEditPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_query::UpdatePlan;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;

impl OfflineWriteSession {
    /// Capture from the same retained exclusive descriptor and require the
    /// complete reviewed BEFORE before reconstructing its original native intent.
    /// No SQL, expression provider or caller-supplied replacement image is used.
    ///
    /// Review must happen before acquiring this session: hosts must not retain
    /// an exclusive lease while waiting for confirmation. This method creates no
    /// journal. Every failure retains the session and its existing receipts.
    pub fn stage_reviewed_update(
        mut self,
        reviewed: UpdatePlan<RecoverableNativeEditPlan>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedUpdate, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_reviewed_update").entered();
        let matched = reviewed.matched_rows();
        let reviewed = reviewed.into_backend();
        let result = (|| {
            self.require_ready()?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeImage::capture(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                budget,
                cancelled,
            )
            .map_err(Cause::Native)?;
            slot.state = SessionState::Ready;

            super::table::require_original(
                image.as_bytes(),
                reviewed.before_bytes(),
                budget,
                cancelled,
            )?;
            reviewed
                .requalify(image, budget, cancelled)
                .map_err(Cause::Qualification)
        })();

        match result {
            Ok(plan) => Ok(StagedUpdate::from_plan(self, plan, matched)),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}
