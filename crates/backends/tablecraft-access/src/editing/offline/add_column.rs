//! Fresh ADD-column reconstruction through the retained exclusive target owner.
use super::plan::MutationPlan;
use super::{Cause, Failure, OfflineWriteSession, Phase, Receipt, SchemaChange, SessionState};
use std::path::Path;
use tablecraft_ace::{NativeImage, RecoverableAddColumnPlan};
use tablecraft_core::limits::ResourceBudget;

/// A checked schema operation; journal setup begins only on consuming apply.
pub struct StagedAddColumn {
    session: OfflineWriteSession,
    plan: RecoverableAddColumnPlan,
}

impl OfflineWriteSession {
    /// Reconstruct the original ADD-column request against this retained target.
    /// Native admission and complete source/image proof consume separate pools.
    /// Refusal retains this owner without creating a journal namespace.
    pub fn stage_reviewed_add_column_with_budgets(
        mut self,
        reviewed: RecoverableAddColumnPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedAddColumn, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_reviewed_add_column").entered();
        let result = (|| {
            self.require_ready()?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeImage::capture(
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
                .map_err(Cause::AddColumnIntent)
        })();
        match result {
            Ok(plan) => Ok(StagedAddColumn {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl StagedAddColumn {
    /// Exact original table spelling retained by the checked request.
    pub fn table(&self) -> &str {
        self.plan.table()
    }

    /// Exact appended column spelling, never a replacement serialized schema.
    pub fn column(&self) -> &str {
        self.plan.column()
    }

    /// Abandon staging while retaining the same exclusive target capability.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Commit the checked original request through its V1 journal and lifecycle.
    /// Schema receipts carry no invented matched or changed record counts.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        super::apply::apply(
            self.session,
            MutationPlan::AddColumn(self.plan),
            (None, None),
            Some(SchemaChange::ColumnAdded),
            bundle,
            budget,
            cancelled,
        )
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
