//! Typed table editing over the same retained offline target and normal planner.
use super::{Cause, Failure, OfflineWriteSession, Phase, SessionState, StagedUpdate};
use crate::editing::journal::NativeJournalEditor;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, Snapshot, UpdateProfile,
};

const IMAGE_COMPARE_CHUNK_BYTES: usize = 64 * 1024;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod original_tests;

/// A real snapshot and normal journal editor, retaining the session's exclusive
/// target while callers read rows and build snapshot-local typed replacements.
/// No source path is reopened and no engineering image is converted. Native
/// program and physical admission remain in the planner. Hosts must not wait for
/// an external approval while holding this offline lease.
pub struct OfflineTableEditor {
    pub(super) session: OfflineWriteSession,
    pub(super) editor: NativeJournalEditor,
}

impl OfflineWriteSession {
    /// Capture from the SAME target descriptor and open the existing normal
    /// typed journal editor. The returned Snapshot owns actual scalar data and
    /// issues fresh local handles. A capture failure requires explicit refresh;
    /// any failure retains the session and its earlier completed receipt.
    pub fn edit_table(
        self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OfflineTableEditor, Failure> {
        self.edit_table_with_original(table, None, budget, cancelled)
    }

    /// Require exact equality with the complete viewed image before issuing
    /// fresh snapshot-local handles. Comparison and editor construction use the
    /// SAME capture from the SAME exclusively retained target descriptor.
    /// No journal is created on mismatch; failure retains the session owner.
    pub fn edit_table_if_original(
        self,
        table: &str,
        expected: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OfflineTableEditor, Failure> {
        self.edit_table_with_original(table, Some(expected), budget, cancelled)
    }

    fn edit_table_with_original(
        mut self,
        table: &str,
        expected: Option<&[u8]>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OfflineTableEditor, Failure> {
        let _span = tracing::debug_span!("access.offline.edit_table").entered();
        let result = (|| {
            self.require_ready()?;
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
            if let Some(expected) = expected {
                require_original(image.as_bytes(), expected, budget, cancelled)?;
            }
            NativeJournalEditor::new(image, table, budget, cancelled).map_err(Cause::Editor)
        })();

        match result {
            Ok(editor) => Ok(OfflineTableEditor {
                session: self,
                editor,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

pub(super) fn require_original(
    actual: &[u8],
    expected: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Cause> {
    super::check(budget, cancelled).map_err(Cause::Native)?;
    if actual.len() != expected.len() {
        return Err(Cause::SourceChanged);
    }

    // Both byte streams are visited. Charge before comparing, including an
    // early mismatch; chunk boundaries provide bounded cancellation latency.
    budget
        .charge(Resource::Bytes, 2 * actual.len() as u64)
        .map_err(InspectError::from)
        .map_err(Cause::Native)?;
    for (actual, expected) in actual
        .chunks(IMAGE_COMPARE_CHUNK_BYTES)
        .zip(expected.chunks(IMAGE_COMPARE_CHUNK_BYTES))
    {
        super::check(budget, cancelled).map_err(Cause::Native)?;
        if actual != expected {
            return Err(Cause::SourceChanged);
        }
    }
    super::check(budget, cancelled).map_err(Cause::Native)
}

impl OfflineTableEditor {
    /// Explicitly abandon this unprepared snapshot and retain the SAME target
    /// lease. Its issued handles do not bind a later fresh editor's Snapshot.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }
}

impl EditableTable for OfflineTableEditor {
    type Error = Failure;
    type Plan = StagedUpdate;

    fn snapshot(&self) -> &Snapshot {
        self.editor.snapshot()
    }

    fn update_profile(&self) -> UpdateProfile {
        self.editor.update_profile()
    }

    fn plan_updates(
        self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedUpdate, Failure> {
        let _span = tracing::debug_span!("access.offline.plan_typed_update").entered();
        let Self { session, editor } = self;
        let matched = updates.len();

        match editor.plan_updates(column, updates, budget, cancelled) {
            Ok(plan) => Ok(StagedUpdate::from_plan(session, plan, matched)),
            Err(error) => Err(session.fail(Phase::Planning, Cause::Editing(error))),
        }
    }
}

impl EditableBatch for OfflineTableEditor {
    fn plan_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedUpdate, Failure> {
        let _span = tracing::debug_span!("access.offline.plan_typed_columns").entered();
        let Self { session, editor } = self;
        match editor.plan_columns_with_count(columns, budget, cancelled) {
            Ok((plan, matched)) => Ok(StagedUpdate::from_plan(session, plan, matched)),
            Err(error) => Err(session.fail(Phase::Planning, Cause::Editing(error))),
        }
    }
}
