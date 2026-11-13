//! Original QueryDef creation on the same exclusive offline source.
use super::{Cause, Failure, OfflineWriteSession, Phase, QueryChange, Receipt, plan::MutationPlan};
use std::path::Path;
use tablecraft_ace::{QueryCreationIntentError, RecoverableNativeQueryCreationPlan};
use tablecraft_core::{limits::ResourceBudget, values::LegacyDateTime};

/// Reviewed new-query request; no durable namespace exists before apply.
pub struct StagedQueryCreation {
    session: OfflineWriteSession,
    plan: RecoverableNativeQueryCreationPlan,
}

impl OfflineWriteSession {
    /// Reconstruct the exact reviewed original against this exclusive target.
    /// Native work and full BEFORE/AFTER comparison retain separate cumulative
    /// caller pools. Hosts acquire this session only after human review.
    ///
    /// # Errors
    /// Refuses changed sources, identity/proof mismatches, cancellation and
    /// budgets while retaining the same resource owner without a new journal.
    pub fn stage_reviewed_query_creation_with_budgets(
        mut self,
        reviewed: RecoverableNativeQueryCreationPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedQueryCreation, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_query_creation").entered();
        let result = self
            .capture_schema_source(None, super::SchemaCapture::Retained, native, cancelled)
            .and_then(|image| {
                reviewed
                    .requalify_with_budgets(image, native, proof, cancelled)
                    .map_err(|error| match error {
                        QueryCreationIntentError::BeforeMismatch => Cause::SourceChanged,
                        other => Cause::QueryCreationIntent(other),
                    })
            });
        match result {
            Ok(plan) => Ok(StagedQueryCreation {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl StagedQueryCreation {
    /// Exact newly created native name.
    pub fn name(&self) -> &str {
        self.plan.intent().name()
    }

    /// Exact result alias captured by the original declaration.
    pub fn alias(&self) -> &str {
        self.plan.intent().alias()
    }

    /// Original signed LONG literal; not an evaluated query result.
    pub fn value(&self) -> i32 {
        self.plan.intent().value()
    }

    /// Explicit captured native creation/update DATE fact.
    pub fn captured_at(&self) -> LegacyDateTime {
        self.plan.intent().captured_at()
    }

    /// Signed identity selected and rechecked by the native allocator.
    pub fn native_query_id(&self) -> i32 {
        self.plan.native_query_id()
    }

    /// Discard staging while retaining the actual exclusive target capability.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Commit only this TCQC creation through its own checked V3 image pair.
    /// Query completion has no table schema or affected-record counts.
    ///
    /// # Errors
    /// Retains the same session and actual journal/setup/recovery owner on error.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        super::apply::apply(
            self.session,
            MutationPlan::QueryCreation(self.plan),
            super::apply::CompletionFacts::Query(Some(QueryChange::Created)),
            bundle,
            budget,
            cancelled,
        )
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
