//! Snapshot-local request conversion for explicitly recoverable native edits.
mod deleting;
mod fitted;
mod inserting;

pub use inserting::{NativeJournalInsertPlan, NativeJournalInserter};
mod fixed;

use super::{NativeEditError, NativeEditor};
use tablecraft_ace::{NativeImage, RecoverableEditError, RecoverableNativeEditPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, Snapshot, UpdateProfile,
};
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
    route: JournalRoute,
}

/// A private routing choice over the same immutable image, never an absence
/// certificate. Every successful native plan reconstructs its own admission.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum JournalRoute {
    Existing,
    Ordinary,
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
        let (profile, route) = editor.journal_profile(budget, cancelled)?;
        Ok(Self {
            editor,
            profile,
            route,
        })
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
        if !self.legacy_int32(column, updates, budget, cancelled)? {
            return self
                .plan_columns_with_count(&[ColumnUpdates { column, updates }], budget, cancelled)
                .map(|(plan, _)| plan);
        }
        let native = self
            .editor
            .int32_updates(column, updates, budget, cancelled)?;
        Ok(self
            .editor
            .plan_recoverable(column, native, budget, cancelled)?)
    }
}

impl EditableBatch for NativeJournalEditor {
    fn plan_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.plan_columns_with_count(columns, budget, cancelled)
            .map(|(plan, _)| plan)
    }
}
