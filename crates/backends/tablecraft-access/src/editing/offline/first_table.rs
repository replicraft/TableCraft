//! The first persisted schema uses the retained native target and V2 journal.
use super::plan::MutationPlan;
use super::table::require_original;
use super::{Cause, Failure, OfflineWriteSession, Phase, Receipt, SchemaChange, SessionState};
use std::path::Path;
use tablecraft_ace::{FirstTableDefinition, FirstTablePlan, NativeImage};
use tablecraft_core::limits::ResourceBudget;

/// Source-bound schema operation; its row counts are deliberately absent.
/// No journal exists until the explicit consuming apply operation.
pub struct StagedFirstTable {
    session: OfflineWriteSession,
    plan: FirstTablePlan,
}

impl OfflineWriteSession {
    /// Construct the first table only from the exact canonical blank profile.
    /// The original snapshot comes from the same retained exclusive descriptor.
    pub fn plan_first_table(
        self,
        definition: FirstTableDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedFirstTable, Failure> {
        self.plan_first_table_with_original(definition, None, budget, cancelled)
    }

    /// Require the complete host-reviewed original before qualifying the schema.
    /// A mismatch retains the same owner and creates no journal or target writes.
    pub fn plan_first_table_if_original(
        self,
        definition: FirstTableDefinition,
        expected: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedFirstTable, Failure> {
        self.plan_first_table_with_original(definition, Some(expected), budget, cancelled)
    }

    fn plan_first_table_with_original(
        mut self,
        definition: FirstTableDefinition,
        expected: Option<&[u8]>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedFirstTable, Failure> {
        let _span = tracing::debug_span!("access.offline.plan_first_table").entered();
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
            if let Some(expected) = expected {
                require_original(image.as_bytes(), expected, budget, cancelled)?;
            }
            image
                .plan_first_table(definition, budget, cancelled)
                .map_err(Cause::FirstTablePlanning)
        })();

        match result {
            Ok(plan) => Ok(StagedFirstTable {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl StagedFirstTable {
    /// The exact validated name retained by the native schema definition.
    pub fn table(&self) -> &str {
        self.plan.table()
    }

    /// Declared scalar fields, including the generated primary ID field.
    pub fn column_count(&self) -> usize {
        self.plan.column_count()
    }

    /// Abandon preparation while retaining the same target capability.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Resize and mutate the same file through its checked native V2 journal.
    /// Failures retain all recovery/finalization resources. No replacement
    /// pathname or fresh-output publisher is involved.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        let schema = SchemaChange::FirstTableCreated {
            column_count: self.plan.column_count(),
        };
        super::apply::apply(
            self.session,
            MutationPlan::FirstTable(self.plan),
            (None, None),
            Some(schema),
            bundle,
            budget,
            cancelled,
        )
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
