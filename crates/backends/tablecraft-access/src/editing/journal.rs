//! Snapshot-local request conversion for explicitly recoverable native edits.

use super::{NativeEditError, NativeEditor};
use tablecraft_ace::{NativeImage, RecoverableEditError, RecoverableNativeEditPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{CellUpdate, EditableTable, Snapshot, UpdateProfile};
use thiserror::Error;

/// No durable operation occurs while constructing either kind of plan.
#[derive(Debug, Error)]
pub enum Error {
    /// Snapshot identity, carrier conversion or native inspection failed.
    #[error(transparent)]
    Native(#[from] NativeEditError),
    /// Normal native planning or bounded intent construction failed.
    #[error(transparent)]
    Plan(#[from] RecoverableEditError),
}

/// A normal native editor whose finish also retains resolved original requests.
/// This is a separate capability: ordinary editors keep their existing plan
/// type and costs. Hosts still own exclusivity, pairing and journal persistence.
pub struct NativeJournalEditor {
    editor: NativeEditor,
    profile: UpdateProfile,
}

impl NativeJournalEditor {
    /// Capture actual readable scalar cells and issue snapshot-local row handles.
    /// Complex and cached declarations are excluded rather than fabricated as
    /// NULL. This does not open a file or prepare a journal.
    pub fn new(
        image: NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeEditError> {
        let editor = NativeEditor::new_journal(image, table, budget, cancelled)?;
        let profile = editor.journal_profile(budget, cancelled)?;
        Ok(Self { editor, profile })
    }
}

impl EditableTable for NativeJournalEditor {
    type Error = Error;
    type Plan = RecoverableNativeEditPlan;

    fn snapshot(&self) -> &Snapshot {
        self.editor.snapshot()
    }

    fn update_profile(&self) -> UpdateProfile {
        self.profile
    }

    fn plan_updates(
        self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        let native = self
            .editor
            .int32_updates(column, updates, budget, cancelled)?;
        Ok(self
            .editor
            .plan_recoverable(column, native, budget, cancelled)?)
    }
}
