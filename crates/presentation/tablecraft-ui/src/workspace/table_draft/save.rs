//! Exact declaration requests and retained outcomes for first-table persistence.
mod dialog;
mod primary;

use super::{CloseIntent, State};
use crate::workspace::objects::memory::Counter;
use crate::{Database, InsertInput, ObjectKind, ObjectTab, ObjectTarget};
use std::sync::atomic::{AtomicU64, Ordering};
use tablecraft_builders::table::TableDraftSnapshot;

const MAX_SAVE_ERROR_BYTES: usize = 4096;
static NEXT_SAVE_REQUEST: AtomicU64 = AtomicU64::new(0);

/// Consuming save intent. Its snapshot carries declarations, never native authority.
pub struct TableSaveRequest {
    pub id: u64,
    pub tab: ObjectTab,
    pub snapshot: TableDraftSnapshot,
    pub record_revision: u64,
    pub property_revision: u64,
}

/// Original unsaved record intent paired with its exact declared table schema.
pub struct InitialRecord {
    pub snapshot: TableDraftSnapshot,
    pub inputs: Vec<InsertInput>,
    /// Exact retained editor text, including text hidden by Omitted or NULL.
    pub raw_text: Vec<String>,
}

impl InitialRecord {
    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        count.add(self.snapshot.retained_bytes());
        count.vector(&self.inputs);
        for input in &self.inputs {
            if let InsertInput::Value(value) = input {
                count.string(&value.text);
            }
        }
        count.vector(&self.raw_text);
        for text in &self.raw_text {
            count.string(text);
        }
        count.finish()
    }
}

pub struct TableDraftSaveCompletion {
    pub target: ObjectTarget,
    pub close_after_save: bool,
    pub resume_transition: bool,
    pub record_pending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Naming,
    ChoosingKey,
    Issued,
    Pending,
    Retained,
}

pub(in crate::workspace) struct SaveDialog {
    tab: ObjectTab,
    revision: u64,
    record_revision: u64,
    property_revision: u64,
    name: String,
    id: Option<u64>,
    phase: Phase,
    close: Option<CloseIntent>,
    error: Option<String>,
    focus_name: bool,
    cancel_requested: bool,
    can_recover: bool,
    can_release: bool,
    can_acknowledge: bool,
    recovery_requested: bool,
}

impl State {
    pub(in crate::workspace) fn table_draft_name(&self, tab: &ObjectTab) -> Option<&str> {
        if let Some(save) = &self.table_draft_save
            && save.tab == *tab
        {
            return Some(&save.name);
        }
        self.table_draft_editor(tab)
            .map(|editor| editor.save_name.as_str())
    }

    pub fn can_save_table_draft(&self) -> bool {
        !self.native_mutation_blocked()
            && !self.has_modified_saved_query_sql()
            && self.table_save_available
            && self.object_switch_ready()
            && !self.creation_is_pending()
            && self.active_object().is_some_and(|tab| {
                self.table_draft_editor(&tab).is_some_and(|editor| {
                    !editor.save_blocked
                        && (!editor.draft.fields().is_empty() || editor.new_field.has_name())
                        && !editor.properties.rejected()
                        && !editor.descriptions.rejected()
                })
            })
    }

    pub fn request_table_draft_save(&mut self) -> bool {
        if !self.can_save_table_draft() {
            return false;
        }
        self.start_table_draft_save(None)
    }

    pub(in crate::workspace) fn start_table_draft_save(
        &mut self,
        close: Option<CloseIntent>,
    ) -> bool {
        if self.native_mutation_blocked()
            || self.has_modified_saved_query_sql()
            || !self.table_save_available
            || self.busy
            || self.creation_is_pending()
            || self.table_draft_save.is_some()
            || self.table_draft_field_delete_open()
        {
            return false;
        }
        let tab = match &close {
            // NO_CLONE: the retained command or exact-owner witness needs its own captured
            // identity after this render/dispatch; keep the original catalog, input
            // or owner available for the later checks in this operation.
            Some(CloseIntent::Tab(tab)) => Some(tab.clone()),
            Some(CloseIntent::All) => self
                .object_tabs()
                .into_iter()
                .find(|tab| self.table_draft_editor(tab).is_some()),
            None => self.active_object(),
        };
        let Some(tab) = tab else {
            return false;
        };
        let Some(editor) = self.table_draft_editor_mut(&tab) else {
            return false;
        };
        if editor.save_blocked
            || !editor.commit_pending()
            || editor.draft.fields().is_empty()
            || !editor.record.synchronize(&editor.draft)
            || editor.record.has_rejected_input()
            || editor.properties.rejected()
            || editor.descriptions.rejected()
        {
            return false;
        }
        self.table_draft_save = Some(SaveDialog {
            tab,
            revision: editor.draft.revision(),
            record_revision: editor.record.revision(),
            property_revision: editor.property_revision(),
            // NO_CLONE: the retained command or exact-owner witness needs its own captured
            // identity after this render/dispatch; keep the original catalog, input
            // or owner available for the later checks in this operation.
            name: editor.save_name.clone(),
            id: None,
            phase: Phase::Naming,
            close,
            error: None,
            focus_name: true,
            cancel_requested: false,
            can_recover: false,
            can_release: false,
            can_acknowledge: false,
            recovery_requested: false,
        });
        true
    }

    pub fn table_draft_save_open(&self) -> bool {
        self.table_draft_save.is_some()
    }

    pub fn table_draft_save_pending(&self) -> bool {
        self.table_draft_save.as_ref().is_some_and(|save| {
            matches!(save.phase, Phase::Issued | Phase::Pending | Phase::Retained)
        })
    }

    /// Consume the issued witness before the host starts reviewing or writing.
    pub fn begin_table_draft_save(&mut self, request: &TableSaveRequest) -> bool {
        if self.native_mutation_blocked() {
            return false;
        }
        let Some(save) = &self.table_draft_save else {
            return false;
        };
        if save.id != Some(request.id)
            || save.phase != Phase::Issued
            || save.tab != request.tab
            || save.record_revision != request.record_revision
            || save.property_revision != request.property_revision
            || !self.save_binding(save)
        {
            return false;
        }
        let Some(editor) = self.table_draft_editor(&save.tab) else {
            return false;
        };
        let Ok(current) = editor.draft.snapshot(&save.name) else {
            return false;
        };
        if request.snapshot != current {
            return false;
        }
        self.table_draft_save.as_mut().unwrap().phase = Phase::Pending;
        self.busy = true;
        true
    }

    fn save_binding(&self, save: &SaveDialog) -> bool {
        self.database
            .as_ref()
            .is_some_and(|database| database.document_revision == save.tab.target.document_revision)
            && self.table_draft_revision_for(&save.tab) == Some(save.revision)
            && self.table_draft_editor(&save.tab).is_some_and(|editor| {
                editor.record.revision() == save.record_revision
                    && editor.property_revision() == save.property_revision
                    && !editor.properties.rejected()
                    && !editor.descriptions.rejected()
                    && !editor.record.has_rejected_input()
                    && editor.rename.is_none()
                    && !editor.new_field.pending()
            })
    }

    /// A host-confirmed no-write failure. Unknown/retained outcomes use `retain`.
    pub fn fail_table_draft_save(&mut self, id: u64, message: String) -> bool {
        self.reset_table_save(id, message)
    }

    /// A confirmed terminal cancellation, not merely a cancellation request.
    pub fn cancel_table_draft_save(&mut self, id: u64) -> bool {
        self.reset_table_save(id, self.localizer.text("table-draft-save-cancelled"))
    }

    fn reset_table_save(&mut self, id: u64, message: String) -> bool {
        let Some(save) = &mut self.table_draft_save else {
            return false;
        };
        if save.id != Some(id) || save.phase == Phase::Naming {
            return false;
        }
        save.id = None;
        save.phase = Phase::Naming;
        save.cancel_requested = false;
        save.can_recover = false;
        save.can_release = false;
        save.can_acknowledge = false;
        save.recovery_requested = false;
        save.focus_name = true;
        save.error = Some(if message.len() <= MAX_SAVE_ERROR_BYTES {
            message
        } else {
            self.localizer.text("table-draft-save-failed")
        });
        self.busy = false;
        true
    }

    /// Keep the pending owner while native visibility/recovery remains unresolved.
    pub fn retain_table_draft_save(&mut self, id: u64, message: String) -> bool {
        self.retain_table_draft_save_status(id, message, false, false, false)
    }

    /// Explicit host report update. Capabilities come from its retained save owner.
    pub fn retain_table_draft_save_status(
        &mut self,
        id: u64,
        message: String,
        can_recover: bool,
        can_release: bool,
        can_acknowledge: bool,
    ) -> bool {
        let Some(save) = &mut self.table_draft_save else {
            return false;
        };
        if save.id != Some(id) || save.phase == Phase::Naming {
            return false;
        }
        save.phase = Phase::Retained;
        save.can_recover = can_recover;
        save.can_release = can_release;
        save.can_acknowledge = can_acknowledge;
        save.recovery_requested = false;
        self.busy = true;
        save.error = Some(if message.len() <= MAX_SAVE_ERROR_BYTES {
            message
        } else {
            self.localizer.text("table-draft-save-retained")
        });
        true
    }

    /// Called only after the host acknowledges a released, unresolved terminal
    /// outcome. The draft remains visible but cannot issue another save.
    pub fn can_acknowledge_table_draft_save(&self, id: u64) -> bool {
        self.table_draft_save.as_ref().is_some_and(|save| {
            save.id == Some(id)
                && save.phase == Phase::Retained
                && save.can_acknowledge
                && self.save_binding(save)
        })
    }

    pub fn acknowledge_table_draft_save(&mut self, id: u64) -> bool {
        if !self.can_acknowledge_table_draft_save(id) {
            return false;
        }
        let save = self.table_draft_save.take().unwrap();
        if let Some(editor) = self.table_draft_editor_mut(&save.tab) {
            editor.save_blocked = true;
            editor.save_message = save.error;
            editor.save_name = save.name;
        }
        self.busy = false;
        self.table_draft_close = save.close;
        true
    }

    pub(in crate::workspace) fn can_save_table_draft_close(&self) -> bool {
        if self.native_mutation_blocked()
            || !self.table_save_available
            || self.busy
            || self.table_draft_save.is_some()
        {
            return false;
        }
        match &self.table_draft_close {
            Some(CloseIntent::Tab(tab)) => self.table_draft_editor(tab).is_some_and(|editor| {
                !editor.save_blocked
                    && (!editor.draft.fields().is_empty() || editor.new_field.has_name())
                    && !editor.properties.rejected()
                    && !editor.descriptions.rejected()
            }),
            Some(CloseIntent::All) => self.object_tabs().iter().any(|tab| {
                self.table_draft_editor(tab).is_some_and(|editor| {
                    !editor.save_blocked
                        && (!editor.draft.fields().is_empty() || editor.new_field.has_name())
                        && !editor.properties.rejected()
                        && !editor.descriptions.rejected()
                })
            }),
            None => false,
        }
    }

    /// Adopt only actual metadata from a host's successful guarded native reopen.
    /// This does not synthesize a catalog entry or infer success from a worker exit.
    pub fn complete_table_draft_save(
        &mut self,
        id: u64,
        database: Database,
    ) -> Option<TableDraftSaveCompletion> {
        let save = self.table_draft_save.as_ref()?;
        if save.id != Some(id)
            || !matches!(save.phase, Phase::Pending | Phase::Retained)
            || !self.save_binding(save)
            || database.document_revision == save.tab.target.document_revision
            || !database
                .tables
                .iter()
                .any(|table| table.name == save.name && table.local && !table.system)
        {
            return None;
        }
        let target = ObjectTarget {
            document_revision: database.document_revision,
            kind: ObjectKind::Table {
                // NO_CLONE: ObjectTarget needs its own captured identity after this
                // render/dispatch; keep the original catalog, input or owner
                // available for the later checks in this operation.
                name: save.name.clone(),
            },
        };
        let editor = self.table_draft_editor(&save.tab)?;
        let continuation = if editor.record.is_modified() {
            let snapshot = editor.draft.snapshot(&save.name).ok()?;
            let inputs = editor.record.inputs(&snapshot)?;
            let raw_text = editor.record.raw_texts(&snapshot)?;
            Some(
                self.prepare_initial_record(
                    &target,
                    InitialRecord {
                        snapshot,
                        inputs,
                        raw_text,
                    },
                )
                .ok()?,
            )
        } else {
            None
        };
        let record_pending = continuation.is_some();
        let save = self.table_draft_save.take().unwrap();
        if let Some(continuation) = continuation {
            self.insert_continuation = Some(continuation);
        }
        let close_after_save = matches!(save.close, Some(CloseIntent::Tab(_)));
        let resume_transition = matches!(save.close, Some(CloseIntent::All));
        self.busy = false;
        self.table_draft_close = None;
        self.table_draft_discard_approved = None;
        self.discard_all_table_drafts();
        self.clear_object_slots();
        self.database = Some(database);
        self.synchronize_objects();
        self.dismiss_backstage();
        Some(TableDraftSaveCompletion {
            target,
            close_after_save,
            resume_transition,
            record_pending,
        })
    }

    pub(in crate::workspace) fn queue_table_draft_save(&mut self) -> bool {
        if !self.can_save_table_draft() {
            return false;
        }
        let Some(editor) = &mut self.table_draft else {
            return false;
        };
        // Header and record widgets must consume this frame's exact text before
        // naming captures either revision. This does not issue a native action.
        editor.save_requested = true;
        true
    }
}

impl SaveDialog {
    pub(in crate::workspace) fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.name.capacity() + self.error.as_ref().map_or(0, String::capacity)
    }
}
