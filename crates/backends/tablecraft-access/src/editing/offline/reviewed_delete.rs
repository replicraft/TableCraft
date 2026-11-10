//! Fresh sealed SQL DELETE qualification over the retained exclusive target.
use super::{Cause, Failure, OfflineWriteSession, Phase, SessionState, StagedDelete, StagedUpdate};
#[cfg(not(feature = "ace16-create"))]
use crate::editing::NativeEditError;
use tablecraft_ace::{NativeImage, RecoverableNativeDeletePlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_query::DeletePlan;

impl OfflineWriteSession {
    /// Consume a lease-free reviewed candidate without reevaluating its SQL.
    /// Capture and native reconstruction use `native`; complete source/image
    /// comparisons and original intent binding use `proof`. Neither pool is reset.
    /// Failure retains the same session and any receipts, without creating a journal.
    pub fn stage_reviewed_delete_with_budgets(
        mut self,
        reviewed: DeletePlan<RecoverableNativeDeletePlan>,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedDelete, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_reviewed_delete").entered();
        let matched = reviewed.matched_rows();
        let reviewed = reviewed.into_backend();
        let result = (|| {
            self.require_ready()?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let source = slot.target.as_mut().ok_or(Cause::InvalidState)?;
            let image = if reviewed.intent().is_retained() {
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
                proof,
                cancelled,
            )?;
            reviewed
                .requalify_with_budgets(image, native, proof, cancelled)
                .map_err(Cause::Qualification)
        })();
        match result {
            Ok(plan) => Ok(StagedDelete {
                staged: StagedUpdate::from_delete(self, plan, matched),
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
