//! Fresh native qualification of a lease-free reviewed UPDATE, without SQL replay.
use super::{Cause, Failure, OfflineWriteSession, Phase, SessionState, StagedUpdate};
#[cfg(not(feature = "ace16-create"))]
use crate::editing::NativeEditError;
use tablecraft_ace::{NativeImage, NativeUpdateProfile, RecoverableNativeEditPlan};
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
        self,
        reviewed: UpdatePlan<RecoverableNativeEditPlan>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedUpdate, Failure> {
        self.stage_reviewed_update_using(reviewed, budget, None, cancelled)
    }

    /// Requalify a reviewed UPDATE with separate native and preservation pools.
    /// Capture and native reconstruction consume `native`; complete source and
    /// replacement comparisons and original-intent binding consume `proof`.
    /// The adapter source comparison charges two complete image lengths in
    /// proof bytes, in addition to its cancellation/work steps.
    /// Failures retain the same target and session state as the shared-pool API.
    pub fn stage_reviewed_update_with_budgets(
        self,
        reviewed: UpdatePlan<RecoverableNativeEditPlan>,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedUpdate, Failure> {
        self.stage_reviewed_update_using(reviewed, native, Some(proof), cancelled)
    }

    fn stage_reviewed_update_using(
        mut self,
        reviewed: UpdatePlan<RecoverableNativeEditPlan>,
        native: &mut ResourceBudget,
        mut proof: Option<&mut ResourceBudget>,
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
            let source = slot.target.as_mut().ok_or(Cause::InvalidState)?;
            let image = if matches!(
                reviewed.intent().profile(),
                NativeUpdateProfile::Ace16RetainedRows
                    | NativeUpdateProfile::Ace16RetainedOpaqueRows
                    | NativeUpdateProfile::Ace16RetainedCurrencyRows
            ) {
                #[cfg(feature = "ace16-create")]
                {
                    NativeImage::capture_retained(source, slot.scope, native, cancelled)
                }
                #[cfg(not(feature = "ace16-create"))]
                {
                    return Err(Cause::Editor(NativeEditError::Unsupported));
                }
            } else {
                NativeImage::capture(source, slot.scope, native, cancelled)
            }
            .map_err(Cause::Native)?;
            slot.state = SessionState::Ready;

            super::table::require_original(
                image.as_bytes(),
                reviewed.before_bytes(),
                match proof.as_deref_mut() {
                    Some(proof) => proof,
                    None => &mut *native,
                },
                cancelled,
            )?;
            match proof {
                Some(proof) => reviewed.requalify_with_budgets(image, native, proof, cancelled),
                None => reviewed.requalify(image, native, cancelled),
            }
            .map_err(Cause::Qualification)
        })();

        match result {
            Ok(plan) => Ok(StagedUpdate::from_plan(self, plan, matched)),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}
