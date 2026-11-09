//! Host-owned native capabilities accompany shared presentation-only object tabs.
use super::{Desktop, filtering::RestoreFilter};
use tablecraft_runtime::{Command, FilterHandle};
use tablecraft_ui::{
    ObjectKind, ObjectOpen, ObjectTab, ObjectTarget, SavedQueryTarget, WorkspaceLayout,
};

#[derive(Default)]
pub(super) struct Sessions {
    filters: Vec<ParkedFilter>,
    definitions: Vec<RestoreFilter>,
    restore: Option<WorkspaceRestore>,
}

struct ParkedFilter {
    tab: ObjectTab,
    handle: FilterHandle,
    enabled: bool,
    offset: u32,
}

pub(super) struct WorkspaceRestore {
    layout: WorkspaceLayout,
    definitions: Vec<RestoreFilter>,
}

impl Desktop {
    fn reserve_filter_parking(&mut self) -> bool {
        if self.filter.is_none() {
            return true;
        }
        if self.objects.filters.try_reserve_exact(1).is_err() {
            self.state.error = Some(self.state.localizer.text("objects-memory-limit"));
            return false;
        }
        true
    }

    fn take_active_filter(&mut self) -> Option<ParkedFilter> {
        let tab = self.state.active_object()?;
        let handle = self.filter.take()?;
        Some(ParkedFilter {
            tab,
            handle,
            enabled: self.state.table_filter_enabled(),
            offset: self.state.data.as_ref().map_or(0, |data| data.offset),
        })
    }

    fn park_filter(&mut self, filter: Option<ParkedFilter>) {
        if let Some(filter) = filter {
            self.objects.filters.retain(|old| old.tab != filter.tab);
            self.objects.filters.push(filter);
        }
    }

    fn bind_active_filter(&mut self) {
        self.filter = self.state.active_object().and_then(|tab| {
            let position = self
                .objects
                .filters
                .iter()
                .position(|filter| filter.tab == tab)?;
            Some(self.objects.filters.remove(position).handle)
        });
    }

    pub(super) fn open_object(&mut self, target: ObjectTarget) {
        if self.editing.has_save() || !self.reserve_filter_parking() {
            return;
        }
        let filter = self.take_active_filter();
        let Some(open) = self.state.prepare_object_open(target) else {
            self.filter = filter.map(|filter| filter.handle);
            return;
        };
        self.park_filter(filter);
        self.bind_active_filter();
        self.load_object(open);
    }

    pub(super) fn activate_object(&mut self, tab: ObjectTab) {
        if self.state.active_object().as_ref() == Some(&tab) {
            return;
        }
        if self.state.field_addition_open() || self.state.field_rename_open() {
            if self.state.can_request_field_addition_transition(&tab)
                || self.state.can_request_field_rename_transition(&tab)
            {
                let _ = self.guard_table_draft_action(
                    super::table_drafts::DeferredDraftAction::ActivateObject(tab),
                );
            }
            return;
        }
        if self.editing.has_save()
            || !self.state.can_activate_object(&tab)
            || !self.reserve_filter_parking()
        {
            return;
        }
        let filter = self.take_active_filter();
        if !self.state.activate_object(&tab) {
            self.filter = filter.map(|filter| filter.handle);
            return;
        }
        self.park_filter(filter);
        self.bind_active_filter();
        self.inspect_field_addition_availability();
        if let Some(open) = self.state.active_object_load() {
            self.load_object(open);
        }
    }

    pub(super) fn close_object(&mut self, tab: ObjectTab) {
        if self.state.field_addition_open() || self.state.field_rename_open() {
            if self.state.can_request_field_addition_transition(&tab)
                || self.state.can_request_field_rename_transition(&tab)
            {
                let _ = self.guard_table_draft_action(
                    super::table_drafts::DeferredDraftAction::CloseObject(tab),
                );
            }
            return;
        }
        if self.editing.has_save() || !self.state.can_close_object(&tab) {
            return;
        }
        let active = self.state.active_object().as_ref() == Some(&tab);
        if !self.state.close_object(&tab) {
            return;
        }
        self.objects.filters.retain(|filter| filter.tab != tab);
        if let ObjectKind::Table { name } = &tab.target.kind {
            self.objects
                .definitions
                .retain(|definition| definition.table() != name);
        }
        if active {
            self.bind_active_filter();
            if let Some(open) = self.state.active_object_load() {
                self.load_object(open);
            }
        }
    }

    pub(super) fn view_object_schema(&mut self, table: String) {
        let Some(database) = &self.state.database else {
            return;
        };
        let target = ObjectTarget {
            document_revision: database.document_revision,
            kind: ObjectKind::Table {
                name: table.clone(),
            },
        };
        if self.editing.has_save() || !self.reserve_filter_parking() {
            return;
        }
        let filter = self.take_active_filter();
        let Some(open) = self.state.prepare_object_open(target) else {
            self.filter = filter.map(|filter| filter.handle);
            return;
        };
        self.park_filter(filter);
        self.bind_active_filter();
        if let Some(design) = self.state.prepare_object_design(&open.tab) {
            self.load_object(design);
        }
    }

    pub(super) fn view_object_datasheet(&mut self, tab: ObjectTab) {
        if self.editing.has_save()
            || !self.state.can_view_object_datasheet(&tab)
            || !self.reserve_filter_parking()
        {
            return;
        }
        let filter = self.take_active_filter();
        let Some(open) = self.state.prepare_object_datasheet(&tab) else {
            self.filter = filter.map(|filter| filter.handle);
            return;
        };
        self.park_filter(filter);
        self.bind_active_filter();
        self.load_object(open);
    }

    pub(super) fn retry_object(&mut self, tab: ObjectTab) {
        let _span = tracing::debug_span!("desktop object retry").entered();
        if self.editing.has_save() || !self.state.can_retry_object(&tab) {
            return;
        }
        if let Some(open) = self.state.active_object_load() {
            self.load_object(open);
        }
    }

    pub(super) fn load_active_object(&mut self, offset: u32) {
        let Some(mut open) = self.state.active_object_load() else {
            return;
        };
        if let ObjectKind::Table { name } = &open.tab.target.kind {
            let filtered = self
                .objects
                .definitions
                .iter()
                .any(|intent| intent.table() == name);
            if !open.table_design && !filtered {
                if !self.state.set_object_load_offset(&open.tab, offset) {
                    return;
                }
                open.offset = offset;
            }
        }
        self.load_object(open);
    }

    pub(super) fn complete_object_filter_restore(&mut self, table: &str) {
        self.objects
            .definitions
            .retain(|definition| definition.table() != table);
    }

    pub(super) fn load_object(&mut self, open: ObjectOpen) {
        if !open.needs_load {
            return;
        }
        match open.tab.target.kind {
            ObjectKind::Table { name } if open.table_design => {
                self.submit(Command::Describe { table: name });
            }
            ObjectKind::Table { name } => {
                let definition = self
                    .objects
                    .definitions
                    .iter()
                    .position(|definition| definition.table() == name);
                if let Some(position) = definition {
                    // Keep the inert intent until a matching publication is accepted.
                    // A failed or cancelled retry must not become an ordinary page read.
                    let retained = self.objects.definitions[position].clone();
                    self.reapply_object_filter(retained);
                } else {
                    self.submit(Command::Page {
                        table: name,
                        offset: open.offset,
                    });
                }
            }
            ObjectKind::SavedQuery { object_id, name } => {
                self.inspect_saved_query(SavedQueryTarget {
                    document_revision: open.tab.target.document_revision,
                    object_id,
                    name,
                });
            }
            ObjectKind::Sql | ObjectKind::DraftTable { .. } => {}
        }
    }

    pub(super) fn capture_filter_restore_definitions(&mut self) -> Option<Vec<RestoreFilter>> {
        let mut definitions = Vec::new();
        let Some(count) = self
            .objects
            .filters
            .len()
            .checked_add(usize::from(self.filter.is_some()))
        else {
            self.state.error = Some(self.state.localizer.text("objects-memory-limit"));
            return None;
        };
        if definitions.try_reserve_exact(count).is_err() {
            self.state.error = Some(self.state.localizer.text("objects-memory-limit"));
            return None;
        }
        for filter in &self.objects.filters {
            definitions.push(RestoreFilter::capture(
                &filter.handle,
                filter.enabled,
                filter.offset,
            ));
        }
        if let Some(handle) = &self.filter {
            definitions.push(RestoreFilter::capture(
                handle,
                self.state.table_filter_enabled(),
                self.state.data.as_ref().map_or(0, |data| data.offset),
            ));
        }
        // Definitions from still-unloaded restored tabs contain preferences only.
        if definitions
            .try_reserve_exact(self.objects.definitions.len())
            .is_err()
        {
            self.state.error = Some(self.state.localizer.text("objects-memory-limit"));
            return None;
        }
        definitions.extend(self.objects.definitions.iter().cloned());
        Some(definitions)
    }

    /// Retain inert definitions after a witnessed query-only replacement; every
    /// old source-bound native handle is invalidated rather than rebound.
    pub(super) fn retain_filter_restore_definitions(&mut self, definitions: Vec<RestoreFilter>) {
        self.objects = Sessions::default();
        self.objects.definitions = definitions;
        self.filter = None;
    }

    pub(super) fn capture_workspace_restore(&mut self) -> Option<WorkspaceRestore> {
        let definitions = self.capture_filter_restore_definitions()?;
        Some(WorkspaceRestore {
            layout: self.state.capture_workspace_layout(),
            definitions,
        })
    }

    pub(super) fn retain_workspace_restore(&mut self, restore: WorkspaceRestore) {
        self.objects.restore = Some(restore);
    }

    pub(super) fn restore_workspace_after_save(&mut self, table: &str) -> bool {
        self.restore_workspace_after_save_preferring(ObjectKind::Table {
            name: table.to_owned(),
        })
    }

    pub(super) fn restore_workspace_after_query_action(&mut self) -> bool {
        self.restore_workspace_after_save_preferring(ObjectKind::Sql)
    }

    fn restore_workspace_after_save_preferring(&mut self, preferred: ObjectKind) -> bool {
        let Some(restore) = self.objects.restore.take() else {
            return false;
        };
        if !self
            .state
            .restore_workspace_layout(restore.layout, preferred)
        {
            return false;
        }
        self.objects.definitions = restore.definitions;
        true
    }
}

#[cfg(test)]
mod tests;
