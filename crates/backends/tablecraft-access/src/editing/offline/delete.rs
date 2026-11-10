//! Typed and SQL normal DELETE reuse the retained normal mutation lifecycle.
use super::{
    Cause, Failure, OfflineTableEditor, OfflineWriteSession, Phase, Receipt, SessionState,
    StagedUpdate,
};
use crate::editing::{journal::NativeJournalEditor, sql};
use std::path::Path;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{DeletableTable, RowHandle, Snapshot};
use tablecraft_expressions::comparison::Collation;
use tablecraft_sql::query::Query;

/// A qualified normal deletion retaining the SAME target before journal setup.
/// Counts are available for report allocation before any durable effects.
pub struct StagedDelete {
    pub(super) staged: StagedUpdate,
}

impl StagedDelete {
    /// Original selected live roots, available before journal preparation.
    pub const fn matched_rows(&self) -> usize {
        self.staged.matched_rows()
    }

    /// Qualified removed roots, available before journal preparation.
    pub const fn deleted_rows(&self) -> usize {
        self.staged.changed_rows()
    }

    /// Abandon this unprepared plan while retaining the exclusive target lease.
    pub fn into_session(self) -> OfflineWriteSession {
        self.staged.into_session()
    }

    /// Apply through the shared retained pair, retirement, cleanup and refresh.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        self.staged.apply(bundle, budget, cancelled)
    }
}

impl DeletableTable for OfflineTableEditor {
    type Error = Failure;
    type Plan = StagedDelete;

    fn snapshot(&self) -> &Snapshot {
        DeletableTable::snapshot(&self.editor)
    }

    fn plan_deletes(
        self,
        rows: &[RowHandle],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        let _span = tracing::debug_span!("access.offline.plan_typed_delete").entered();
        let Self { session, editor } = self;
        match editor.plan_deletes(rows, budget, cancelled) {
            Ok(plan) => Ok(StagedDelete {
                staged: StagedUpdate::from_delete(session, plan, rows.len()),
            }),
            Err(error) => Err(session.fail(Phase::Planning, Cause::Editing(error))),
        }
    }
}

impl OfflineWriteSession {
    /// Plan a closed local DELETE using the SAME retained target descriptor.
    /// Predicates read the original snapshot; all normal admission is rederived.
    pub fn plan_delete(
        mut self,
        query: &Query,
        budget: &mut ResourceBudget,
        collation: Option<&dyn Collation>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedDelete, Failure> {
        let _span = tracing::debug_span!("access.offline.plan_delete").entered();
        let result = (|| {
            self.require_ready()?;
            let table = sql::local_delete_table(query, false).map_err(|error| {
                Cause::DeletePlanning(sql::RecoverableDeleteError::Shape(error))
            })?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeJournalEditor::capture_image(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                table,
                budget,
                cancelled,
            )
            .map_err(Cause::Native)?;
            slot.state = SessionState::Ready;
            let plan = sql::plan_recoverable_delete(image, query, budget, collation, cancelled)
                .map_err(Cause::DeletePlanning)?;
            let matched = plan.matched_rows();
            Ok((plan.into_backend(), matched))
        })();
        match result {
            Ok((plan, matched)) => Ok(StagedDelete {
                staged: StagedUpdate::from_delete(self, plan, matched),
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}
