//! Atomic populated creation from a reviewed complete native candidate.
use super::{Cause, Failure, OfflineWriteSession, Phase, Receipt, plan::MutationPlan};
use std::path::Path;
use tablecraft_ace::PopulatedAddTablePlan;
use tablecraft_core::limits::ResourceBudget;

/// Retains one source-bound declaration and complete materialized relation.
/// No journal or target write exists before consuming apply.
pub struct StagedPopulatedAddTable {
    session: OfflineWriteSession,
    plan: PopulatedAddTablePlan,
}

impl OfflineWriteSession {
    /// Reconstruct a reviewed populated candidate against this exclusive target.
    /// Native construction and complete image comparison use separate cumulative
    /// caller pools. Failures retain the session without creating a journal.
    pub fn stage_reviewed_populated_add_table_with_budgets(
        mut self,
        reviewed: PopulatedAddTablePlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedPopulatedAddTable, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_populated_table").entered();
        let result = self
            .capture_schema_source(None, super::SchemaCapture::TableAppend, native, cancelled)
            .and_then(|image| {
                reviewed
                    .requalify_with_budgets(image, native, proof, cancelled)
                    .map_err(Cause::Qualification)
            });
        match result {
            Ok(plan) => Ok(StagedPopulatedAddTable {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl StagedPopulatedAddTable {
    /// Actual new table name.
    pub fn table(&self) -> &str {
        self.plan.table()
    }

    /// Number of manual destination fields.
    pub fn column_count(&self) -> usize {
        self.plan.column_count()
    }

    /// Complete selected row count, including zero.
    pub fn inserted_rows(&self) -> usize {
        self.plan.row_count()
    }

    /// Abandon the staged candidate while retaining the exclusive target owner.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Publish schema and every selected row through one original-intent V3 journal.
    /// Matched rows are absent; changed rows count newly inserted destination rows.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        let facts = super::apply::CompletionFacts::PopulatedTable {
            column_count: self.plan.column_count(),
            inserted_rows: self.plan.row_count(),
        };
        super::apply::apply(
            self.session,
            MutationPlan::PopulatedTable(self.plan),
            facts,
            bundle,
            budget,
            cancelled,
        )
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
