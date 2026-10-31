//! Typed table editing over the same retained offline target and normal planner.
use super::{Cause, Failure, OfflineWriteSession, Phase, SessionState, StagedUpdate};
use crate::editing::journal::NativeJournalEditor;
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{CellUpdate, EditableTable, Snapshot, UpdateProfile};

/// A real snapshot and normal journal editor, retaining the session's exclusive
/// target while callers read rows and build snapshot-local typed replacements.
/// No source path is reopened, no engineering image is converted, and ACE's
/// existing normal write profile remains unavailable. Hosts must not wait for
/// an external approval while holding this offline lease.
pub struct OfflineTableEditor {
    session: OfflineWriteSession,
    editor: NativeJournalEditor,
}

impl OfflineWriteSession {
    /// Capture from the SAME target descriptor and open the existing normal
    /// typed journal editor. The returned Snapshot owns actual scalar data and
    /// issues fresh local handles. A capture failure requires explicit refresh;
    /// any failure retains the session and its earlier completed receipt.
    pub fn edit_table(
        mut self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OfflineTableEditor, Failure> {
        let _span = tracing::debug_span!("access.offline.edit_table").entered();
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
