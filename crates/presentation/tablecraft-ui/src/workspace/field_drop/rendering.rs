//! Bounded render snapshots and post-widget readonly dispatch.
use super::{FieldDropTarget, model::Phase};
use crate::workspace::{Action, DocumentView, ObjectKind, ObjectTab, State};

impl State {
    pub(in crate::workspace) fn field_drop_table_entry_ready(&self, tab: &ObjectTab) -> bool {
        self.field_drop_available && self.field_drop.draft.is_none()
            && self.field_drop.warning.is_none() && self.objects.is_active_tab(tab)
            && !self.native_mutation_blocked() && self.object_switch_ready()
            && !self.has_unsaved_table_drafts() && !self.has_modified_saved_query_sql()
            && self.database.as_ref().is_some_and(|database| {
                database.document_revision == tab.target.document_revision
                    && matches!(&tab.target.kind, ObjectKind::Table { name }
                        if database.tables.iter().filter(|table| table.name == *name && table.local && !table.system).count() == 1)
            })
    }

    pub(in crate::workspace) fn field_drop_pending_target(&mut self) -> Option<FieldDropTarget> {
        let (target, _) = self.pending_field_drop()?;
        if !self.field_drop_owner_bound(target) {
            return None;
        }
        let target = target.copy()?;
        let additional = target
            .heap_bytes()?
            .checked_add(size_of::<FieldDropTarget>())?;
        if !self.admit_object_bytes(Some(additional)) {
            return None;
        }
        Some(target)
    }

    pub fn can_delete_selected_saved_field(&self) -> bool {
        if self.view != DocumentView::Design {
            return false;
        }
        let Some(tab) = self.active_object() else {
            return false;
        };
        self.field_drop_table_entry_ready(&tab)
            && self.schema.as_ref().is_some_and(|schema| {
                matches!(&tab.target.kind, ObjectKind::Table { name } if name == &schema.table)
                    && self.selected_field < schema.fields.len()
            })
    }

    pub(in crate::workspace) fn begin_selected_saved_field_drop(&mut self) -> bool {
        if !self.can_delete_selected_saved_field() {
            return false;
        }
        let tab = self.active_object().unwrap();
        let field = &self.schema.as_ref().unwrap().fields[self.selected_field];
        let Some(target) = FieldDropTarget::from_parts(&tab, &field.name, field.number) else {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return false;
        };
        self.begin_field_drop(target)
    }

    pub(in crate::workspace) fn finish_field_drop_inspection(&self, actions: &mut Vec<Action>) {
        if let Some(target) = self.field_drop_inspection_target() {
            actions.push(Action::InspectFieldDrop { target });
        }
    }

    pub(in crate::workspace) fn dismiss_field_drop_warning_details(&mut self) -> bool {
        if self.field_drop.draft.is_some() || self.field_drop.status.is_some() {
            return false;
        }
        let Some(warning) = &mut self.field_drop.warning else {
            return false;
        };
        if !warning.open || warning.owner.phase != Phase::PendingDeletion {
            return false;
        }
        warning.open = false;
        true
    }
}
