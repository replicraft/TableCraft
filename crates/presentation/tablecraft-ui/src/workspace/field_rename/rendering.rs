//! Bounded render snapshots and post-widget text/revision publication.
use super::{FieldRenameTarget, copy_text, widget};
use crate::workspace::{Action, ObjectKind, ObjectTab, State};
use tablecraft_builders::table::MAX_DRAFT_NAME_BYTES;

impl State {
    pub(in crate::workspace) fn field_rename_editor_presentation(
        &mut self,
    ) -> Option<widget::Presentation> {
        if !self.field_rename_available {
            return None;
        }
        let draft = self.field_rename.presented()?;
        if !self.field_rename_owner_bound(&draft.target) {
            return None;
        }
        let target = draft.target.copy()?;
        let proposed = copy_text(&draft.proposed)?;
        let revision = draft.revision;
        let enabled = self.can_edit_field_rename();
        let focus = draft.focus && enabled;
        let additional = target
            .heap_bytes()?
            .checked_add(proposed.capacity())?
            .checked_add(2 * MAX_DRAFT_NAME_BYTES)?
            .checked_add(size_of::<widget::Presentation>())?;
        if !self.admit_object_bytes(Some(additional)) {
            return None;
        }

        Some(widget::Presentation {
            target,
            proposed,
            revision,
            enabled,
            focus,
        })
    }

    pub(in crate::workspace) fn apply_field_rename_widget(
        &mut self,
        target: &FieldRenameTarget,
        enabled: bool,
        event: Option<widget::Event>,
    ) {
        if enabled
            && let Some(draft) = &mut self.field_rename.draft
            && draft.target == *target
        {
            draft.focus = false;
        }
        match event {
            Some(widget::Event::Begin(_)) => {}
            Some(widget::Event::Name { revision, name }) => {
                let current = self
                    .field_rename
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.target == *target && draft.revision == revision);
                if current {
                    self.set_field_rename_name(target, name);
                }
            }
            Some(widget::Event::Cancel { revision }) => {
                self.discard_field_rename(target, revision);
            }
            Some(widget::Event::MemoryLimit) => {
                self.error = Some(self.localizer.text("objects-memory-limit"));
            }
            None => {}
        }
    }

    pub(in crate::workspace) fn field_rename_table_entry_ready(&self, tab: &ObjectTab) -> bool {
        self.field_rename_available
            && self.field_rename.draft.is_none()
            && self.field_rename.warning.is_none()
            && self.objects.is_active_tab(tab)
            && !self.native_mutation_blocked()
            && self.object_switch_ready()
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
            && self.database.as_ref().is_some_and(|database| {
                database.document_revision == tab.target.document_revision
                    && match &tab.target.kind {
                        ObjectKind::Table { name } => {
                            database
                                .tables
                                .iter()
                                .filter(|table| table.name == *name && table.local && !table.system)
                                .count()
                                == 1
                        }
                        _ => false,
                    }
            })
    }
}

impl State {
    pub(in crate::workspace) fn finish_field_rename_inspection(&self, actions: &mut Vec<Action>) {
        if let Some(target) = self.field_rename_inspection_target() {
            actions.push(Action::InspectFieldRename { target });
        }
    }
}
