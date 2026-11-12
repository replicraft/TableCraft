//! A distinct original index DROP request over the same retained exclusive source.
use super::{
    Cause, Failure, OfflineWriteSession, Phase, Receipt, SchemaChange, plan::MutationPlan,
};
use std::path::Path;
use tablecraft_ace::RecoverableNativeIndexDropPlan;
use tablecraft_core::limits::ResourceBudget;

/// Source-requalified ordinary secondary index DROP, without a journal before apply.
pub struct StagedIndexDrop {
    session: OfflineWriteSession,
    plan: RecoverableNativeIndexDropPlan,
}

impl OfflineWriteSession {
    /// Reconstruct the reviewed original request against this exclusive source.
    /// Complete BEFORE/AFTER comparison and native work retain separate pools.
    ///
    /// # Errors
    /// Returns native source, stale-image, proof, cancellation or resource refusal
    /// while retaining the same exclusive target without creating a namespace.
    pub fn stage_reviewed_index_drop_with_budgets(
        mut self,
        reviewed: RecoverableNativeIndexDropPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedIndexDrop, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_index_drop").entered();
        let result = self
            .capture_schema_source(None, super::SchemaCapture::TableAppend, native, cancelled)
            .and_then(|image| {
                reviewed
                    .requalify_with_budgets(image, native, proof, cancelled)
                    .map_err(Cause::IndexDropIntent)
            });
        match result {
            Ok(plan) => Ok(StagedIndexDrop {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl StagedIndexDrop {
    /// Original table name bound by the native request.
    pub fn table(&self) -> &str {
        self.plan.table()
    }

    /// Exact requested index name.
    pub fn index_name(&self) -> &str {
        self.plan.index_name()
    }

    /// Reclaimed tree and map pages; never an affected-record count.
    pub fn retired_pages(&self) -> usize {
        self.plan.retired_pages()
    }

    /// Discard the staged request while retaining its actual source owner.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Remove one index through its original TCID request and exact V3 image.
    /// Both affected-row counts remain absent because table data is unchanged.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        super::apply::apply(
            self.session,
            MutationPlan::IndexDrop(self.plan),
            super::apply::CompletionFacts::Schema(Some(SchemaChange::IndexDropped)),
            bundle,
            budget,
            cancelled,
        )
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
