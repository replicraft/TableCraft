//! A distinct original index request over the same retained exclusive source.
use super::{
    Cause, Failure, OfflineWriteSession, Phase, Receipt, SchemaChange, plan::MutationPlan,
};
use std::path::Path;
use tablecraft_ace::RecoverableNativeIndexCreatePlan;
use tablecraft_core::limits::ResourceBudget;

/// Source-requalified ordinary secondary index, without a journal before apply.
pub struct StagedIndexCreation {
    session: OfflineWriteSession,
    plan: RecoverableNativeIndexCreatePlan,
}

impl OfflineWriteSession {
    /// Reconstruct the reviewed original request against this exclusive source.
    /// Complete BEFORE/AFTER comparison and native work retain separate pools.
    pub fn stage_reviewed_index_creation_with_budgets(
        mut self,
        reviewed: RecoverableNativeIndexCreatePlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedIndexCreation, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_index_creation").entered();
        let result = self
            .capture_schema_source(None, super::SchemaCapture::TableAppend, native, cancelled)
            .and_then(|image| {
                reviewed
                    .requalify_with_budgets(image, native, proof, cancelled)
                    .map_err(Cause::IndexCreationIntent)
            });
        match result {
            Ok(plan) => Ok(StagedIndexCreation {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl StagedIndexCreation {
    /// Original table name bound by the native request.
    pub fn table(&self) -> &str {
        self.plan.table()
    }
    /// Original field name, never a display ordinal.
    pub fn column(&self) -> &str {
        self.plan.column()
    }
    /// Exact requested index name.
    pub fn index_name(&self) -> &str {
        self.plan.index_name()
    }
    /// Source population metadata; not an affected-row count.
    pub fn indexed_rows(&self) -> usize {
        self.plan.indexed_rows()
    }
    /// Discard the staged request while retaining its actual source owner.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }
    /// Commit one index through its original TCIC request and exact V3 image.
    /// Both affected-row counts remain absent because table data is unchanged.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        super::apply::apply(
            self.session,
            MutationPlan::IndexCreation(self.plan),
            super::apply::CompletionFacts::Schema(Some(SchemaChange::IndexCreated)),
            bundle,
            budget,
            cancelled,
        )
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
