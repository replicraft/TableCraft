//! Fresh source-bound table DROP under the retained exclusive target owner.
use super::plan::MutationPlan;
use super::{Cause, Failure, OfflineWriteSession, Phase, Receipt, SchemaChange, SessionState};
use std::path::Path;
use tablecraft_ace::{NativeImage, NativeTableDropIdentity, RecoverableTableDropPlan};
use tablecraft_core::limits::ResourceBudget;

/// A reviewed empty-table DROP; no journal exists until explicit consuming apply.
/// Historical table storage remains allocated; this operation does not reclaim it.
pub struct StagedTableDrop {
    session: OfflineWriteSession,
    plan: RecoverableTableDropPlan,
}

impl OfflineWriteSession {
    /// Reconstruct the original DROP from fresh bytes on this retained target.
    /// Native admission uses `native`; whole-source/image proof uses `proof`.
    /// Refusal retains this same session and creates no journal namespace.
    pub fn stage_reviewed_table_drop_with_budgets(
        mut self,
        reviewed: RecoverableTableDropPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedTableDrop, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_reviewed_table_drop").entered();
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
                .map_err(Cause::TableDropIntent)
        })();

        match result {
            Ok(plan) => Ok(StagedTableDrop {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl StagedTableDrop {
    /// Exact original catalog name retained by the reviewed operation.
    pub fn table(&self) -> &str {
        self.plan.table()
    }

    /// Stored source row count; current native admission requires an empty table.
    pub fn declared_rows(&self) -> u32 {
        self.plan.declared_rows()
    }

    /// Original catalog and definition identity, preserved by the checked request.
    pub fn identity(&self) -> NativeTableDropIdentity {
        self.plan.identity()
    }

    /// Abandon staging while retaining the exact exclusive target capability.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Apply the exact reviewed schema operation through its own checked intent.
    /// Record counts remain absent; failures retain recovery/finalization owners.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        let schema = Some(SchemaChange::TableDropped);
        super::apply::apply(
            self.session,
            MutationPlan::TableDrop(self.plan),
            super::apply::CompletionFacts::Schema(schema),
            bundle,
            budget,
            cancelled,
        )
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
