//! Bounded object sessions move presentation state; they never clone native rows.
mod layout;
pub(super) mod memory;
mod names;
mod ownership;
mod saved_sql;
mod saved_sql_close;
mod saved_sql_lifecycle;
mod session;
mod shortcuts;

pub use layout::WorkspaceLayout;

use super::{DocumentView, State};
use session::{Entry, Session};
use std::sync::atomic::{AtomicU64, Ordering};

pub(in crate::workspace) const MAX_OBJECTS: usize = 16;
pub(in crate::workspace) const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;
static NEXT_OBJECT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Table { name: String },
    SavedQuery { object_id: u128, name: String },
    DraftTable { draft_id: u64 },
    Sql,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjectTarget {
    pub document_revision: u64,
    pub kind: ObjectKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjectTab {
    pub id: u64,
    pub target: ObjectTarget,
}

pub struct ObjectOpen {
    pub tab: ObjectTab,
    pub needs_load: bool,
    pub table_design: bool,
    pub offset: u32,
}

#[derive(Default)]
pub(super) struct Objects {
    entries: Vec<Entry>,
    active: Option<u64>,
    navigation: Option<ObjectTarget>,
    revision: Option<u64>,
}

impl State {
    pub fn object_tabs(&self) -> Vec<ObjectTab> {
        self.objects
            .entries
            .iter()
            .map(|entry| entry.tab.clone())
            .collect()
    }

    pub fn active_object(&self) -> Option<ObjectTab> {
        self.objects
            .entries
            .iter()
            .find(|entry| Some(entry.tab.id) == self.objects.active)
            .map(|entry| entry.tab.clone())
    }

    pub fn selected_navigation_object(&self) -> Option<ObjectTarget> {
        self.objects.navigation.clone()
    }

    pub fn select_navigation_object(&mut self, target: ObjectTarget) -> bool {
        if !self.object_switch_ready() || !self.object_binding(&target) {
            return false;
        }
        self.objects.navigation = Some(target);
        true
    }

    pub fn navigation_visible(&self) -> bool {
        !self.navigation_hidden
    }

    pub fn toggle_navigation(&mut self) {
        if self.object_switch_ready() {
            self.navigation_hidden = !self.navigation_hidden;
        }
    }

    pub fn can_open_object(&self, target: &ObjectTarget) -> bool {
        let existing = self
            .objects
            .entries
            .iter()
            .any(|entry| entry.tab.target == *target);
        let mut additional = memory::Counter::new(if existing {
            0
        } else {
            size_of::<Entry>() + size_of::<Session>()
        });
        if !existing {
            match &target.kind {
                ObjectKind::Table { name } | ObjectKind::SavedQuery { name, .. } => {
                    additional.string(name);
                    additional.string(name);
                }
                ObjectKind::DraftTable { .. } | ObjectKind::Sql => {}
            }
        }
        self.object_switch_ready()
            && self.object_binding(target)
            && (existing || self.objects.entries.len() < MAX_OBJECTS)
            && self
                .object_bytes()
                .and_then(|bytes| bytes.checked_add(additional.finish()?))
                .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES)
    }

    pub fn prepare_object_open(&mut self, target: ObjectTarget) -> Option<ObjectOpen> {
        self.synchronize_objects();
        if !self.can_open_object(&target) {
            self.error = Some(self.localizer.text("objects-open-refused"));
            return None;
        }
        if let Some(tab) = self
            .objects
            .entries
            .iter()
            .find(|entry| entry.tab.target == target)
            .map(|entry| entry.tab.clone())
        {
            return self
                .activate_object(&tab)
                .then(|| self.active_object_load())
                .flatten();
        }
        if self.objects.entries.try_reserve_exact(1).is_err() {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return None;
        }
        let Ok(previous) =
            NEXT_OBJECT_ID.try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        else {
            self.error = Some(self.localizer.text("objects-open-refused"));
            return None;
        };
        let id = previous + 1;
        let tab = ObjectTab {
            id,
            target: target.clone(),
        };
        self.park_object();
        self.objects.entries.push(Entry {
            tab: tab.clone(),
            session: None,
            offset: 0,
            retained_bytes: Some(0),
        });
        self.objects.active = Some(id);
        self.objects.navigation = Some(target.clone());
        let session = Session::empty(&target.kind);
        session.install(self);
        self.finish_object_activation();
        self.active_object_load()
    }

    pub fn can_activate_object(&self, tab: &ObjectTab) -> bool {
        self.object_switch_ready()
            && self.object_binding(&tab.target)
            && self.objects.entries.iter().any(|entry| entry.tab == *tab)
            && self
                .object_bytes()
                .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES)
    }

    pub fn activate_object(&mut self, tab: &ObjectTab) -> bool {
        if !self.can_activate_object(tab) {
            return false;
        }
        if self.objects.active == Some(tab.id) {
            return true;
        }
        self.park_object();
        let entry = self
            .objects
            .entries
            .iter_mut()
            .find(|entry| entry.tab == *tab)
            .expect("checked object");
        entry.retained_bytes = Some(0);
        let session = entry
            .session
            .take()
            .unwrap_or_else(|| Session::empty(&tab.target.kind));
        self.objects.active = Some(tab.id);
        self.objects.navigation = Some(tab.target.clone());
        session.install(self);
        self.finish_object_activation();
        true
    }

    pub fn active_object_load(&self) -> Option<ObjectOpen> {
        let tab = self.active_object()?;
        let (needs_load, table_design, offset) = match &tab.target.kind {
            ObjectKind::Table { .. } => {
                let design = self.view == DocumentView::Design;
                (
                    if design {
                        self.schema.is_none()
                    } else {
                        self.data.is_none()
                    },
                    design,
                    self.data.as_ref().map_or_else(
                        || {
                            self.objects
                                .entries
                                .iter()
                                .find(|entry| entry.tab == tab)
                                .map_or(0, |entry| entry.preferred_offset())
                        },
                        |data| data.offset,
                    ),
                )
            }
            ObjectKind::SavedQuery { .. } => (self.saved_query.needs_inspection(), false, 0),
            ObjectKind::DraftTable { .. } | ObjectKind::Sql => (false, false, 0),
        };
        Some(ObjectOpen {
            tab,
            needs_load,
            table_design,
            offset,
        })
    }

    pub fn can_view_object_datasheet(&self, tab: &ObjectTab) -> bool {
        self.can_select_table_object_view(tab)
    }

    pub fn prepare_object_datasheet(&mut self, tab: &ObjectTab) -> Option<ObjectOpen> {
        if !self.can_view_object_datasheet(tab) || !self.activate_object(tab) {
            return None;
        }
        if matches!(tab.target.kind, ObjectKind::DraftTable { .. }) {
            if !self.show_table_draft_datasheet() {
                return None;
            }
        } else {
            self.view = DocumentView::Table;
            self.finish_object_activation();
        }
        self.active_object_load()
    }

    pub fn can_view_object_design(&self, tab: &ObjectTab) -> bool {
        self.can_select_table_object_view(tab)
    }

    fn can_select_table_object_view(&self, tab: &ObjectTab) -> bool {
        self.can_activate_object(tab)
            && match tab.target.kind {
                ObjectKind::Table { .. } => true,
                ObjectKind::DraftTable { .. } => {
                    !self.creation_is_pending()
                        && self
                            .table_draft_editor(tab)
                            .is_some_and(|editor| editor.view_available())
                }
                _ => false,
            }
    }

    pub fn prepare_object_design(&mut self, tab: &ObjectTab) -> Option<ObjectOpen> {
        if !self.can_view_object_design(tab) || !self.activate_object(tab) {
            return None;
        }
        if matches!(tab.target.kind, ObjectKind::DraftTable { .. }) {
            if !self.show_table_draft_design() {
                return None;
            }
        } else {
            self.view = DocumentView::Design;
            self.finish_object_activation();
            self.schema_activation_pending = self.schema.is_none();
        }
        self.active_object_load()
    }

    /// Remember a requested page position without creating any row identity.
    pub fn set_object_load_offset(&mut self, tab: &ObjectTab, offset: u32) -> bool {
        if !matches!(tab.target.kind, ObjectKind::Table { .. })
            || !self.can_activate_object(tab)
            || self.active_object().as_ref() != Some(tab)
            || self.data.is_some()
        {
            return false;
        }
        let Some(entry) = self
            .objects
            .entries
            .iter_mut()
            .find(|entry| entry.tab == *tab)
        else {
            return false;
        };
        entry.offset = offset;
        true
    }

    pub fn can_retry_object(&self, tab: &ObjectTab) -> bool {
        self.can_activate_object(tab)
            && self.active_object().as_ref() == Some(tab)
            && self
                .active_object_load()
                .is_some_and(|open| open.needs_load)
    }

    pub fn can_close_object(&self, tab: &ObjectTab) -> bool {
        self.object_switch_ready()
            && self.object_binding(&tab.target)
            && self.objects.entries.iter().any(|entry| entry.tab == *tab)
    }

    pub fn close_object(&mut self, tab: &ObjectTab) -> bool {
        if !self.can_close_object(tab) {
            return false;
        }
        if self.saved_query_sql_revision_for(tab).is_some() {
            self.request_saved_query_sql_close(tab);
            return false;
        }
        let approved = self.table_draft_discard_approved.as_ref().is_some_and(
            |(approved, revision, record_revision, property_revision)| {
                approved == tab
                    && self.table_draft_revision_for(tab) == Some(*revision)
                    && self.table_draft_record_revision_for(tab) == Some(*record_revision)
                    && self.table_draft_property_revision_for(tab) == Some(*property_revision)
            },
        );
        if self.modified_table_draft(tab) && !approved {
            self.table_draft_discard_approved = None;
            self.table_draft_close = Some(super::table_draft::CloseIntent::Tab(tab.clone()));
            return false;
        }
        self.table_draft_discard_approved = None;
        self.close_object_confirmed(tab)
    }

    pub(super) fn close_object_confirmed(&mut self, tab: &ObjectTab) -> bool {
        if self.saved_query_sql_revision_for(tab).is_some() {
            return false;
        }
        let Some(index) = self
            .objects
            .entries
            .iter()
            .position(|entry| entry.tab == *tab)
        else {
            return false;
        };
        let active = self.objects.active == Some(tab.id);
        self.objects.entries.remove(index);
        if tab.target.kind == ObjectKind::Sql {
            self.sql.clear();
        }
        if active {
            self.clear_object_slots();
            self.objects.active = None;
            let next = self
                .objects
                .entries
                .get(index.min(self.objects.entries.len().saturating_sub(1)))
                .map(|entry| entry.tab.clone());
            if let Some(next) = next {
                self.activate_object(&next);
            }
            self.finish_object_activation();
        }
        true
    }

    pub(super) fn object_switch_ready(&self) -> bool {
        !self.number_column_open() && self.object_switch_ready_without_number_column()
    }

    pub(super) fn object_switch_ready_without_number_column(&self) -> bool {
        !self.busy
            && !self.saved_query_sql_close_open()
            && self.table_draft_close.is_none()
            && self.table_draft_save.is_none()
            && !self.table_draft_field_delete_open()
            && !self.backstage_visible()
            && !self.edit_dialog_open()
            && !self.insertion_dialog_open()
            && !self.deletion_dialog_open()
            && self.contents_request_id().is_none()
            && !self.parameter_prompt_open()
            && !self.query_action_dialog_open()
            && !self.table_rename_open()
            && !self.find_window_open()
            && !self.zoom_dialog_open()
            && !self.table_filter_pending()
            && !self.edit_save_pending()
    }

    fn object_binding(&self, target: &ObjectTarget) -> bool {
        self.database.as_ref().is_some_and(|database| {
            database.document_revision == target.document_revision
                && match &target.kind {
                    ObjectKind::Table { name } => {
                        database.tables.iter().any(|table| table.name == *name)
                    }
                    ObjectKind::SavedQuery { object_id, name } => database
                        .queries
                        .iter()
                        .any(|query| query.object_id == *object_id && query.name == *name),
                    ObjectKind::DraftTable { draft_id } => self.objects.entries.iter().any(|entry| {
                        matches!(entry.tab.target.kind, ObjectKind::DraftTable { draft_id: current } if current == *draft_id)
                    }),
                    ObjectKind::Sql => true,
                }
        })
    }

    pub(super) fn synchronize_objects(&mut self) {
        let revision = self
            .database
            .as_ref()
            .map(|database| database.document_revision);
        if self.objects.revision != revision {
            if self.has_modified_saved_query_sql() {
                return;
            }
            self.objects = Objects {
                revision,
                ..Objects::default()
            };
        }
        if self.objects.active.is_some() {
            return;
        }
        let Some(document_revision) = revision else {
            return;
        };
        let table = self
            .data
            .as_ref()
            .filter(|data| data.source_revision == Some(document_revision))
            .map(|data| ObjectKind::Table {
                name: data.table.clone(),
            })
            .or_else(|| {
                self.schema.as_ref().map(|schema| ObjectKind::Table {
                    name: schema.table.clone(),
                })
            });
        let saved = self
            .saved_query
            .target
            .as_ref()
            .map(|target| ObjectKind::SavedQuery {
                object_id: target.object_id,
                name: target.name.clone(),
            });
        let active_kind = match self.view {
            DocumentView::Table | DocumentView::Design => table.clone(),
            DocumentView::Query if self.saved_query.active => saved.clone(),
            DocumentView::Query => Some(ObjectKind::Sql),
        };
        let has_sql_result = self.query_data.is_some();
        let mut kinds: Vec<ObjectKind> = table.into_iter().chain(saved).collect();
        if has_sql_result || active_kind == Some(ObjectKind::Sql) {
            kinds.push(ObjectKind::Sql);
        }
        for kind in kinds {
            let target = ObjectTarget {
                document_revision,
                kind,
            };
            if !self.object_binding(&target) {
                continue;
            }
            let Ok(previous) =
                NEXT_OBJECT_ID
                    .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            else {
                self.error = Some(self.localizer.text("objects-open-refused"));
                return;
            };
            if self.objects.entries.try_reserve_exact(1).is_err() {
                self.error = Some(self.localizer.text("objects-memory-limit"));
                return;
            }
            let active = active_kind.as_ref() == Some(&target.kind);
            let session = if active {
                None
            } else {
                let error = self.error.take();
                let session = Session::take(self, &target.kind);
                self.error = error;
                Some(session)
            };
            let tab = ObjectTab {
                id: previous + 1,
                target,
            };
            if active {
                self.objects.active = Some(tab.id);
            }
            let retained_bytes = session.as_ref().map_or(Some(0), Session::retained_bytes);
            self.objects.entries.push(Entry {
                tab,
                session,
                offset: 0,
                retained_bytes,
            });
        }
    }

    pub(super) fn adopt_table_publication(&mut self, data: &super::Data) -> bool {
        let Some(active) = self.active_object() else {
            return self.database.as_ref().is_none_or(|database| {
                data.source_revision
                    .is_none_or(|revision| revision == database.document_revision)
            });
        };
        let matches = data.source_revision == Some(active.target.document_revision)
            && matches!(&active.target.kind, ObjectKind::Table { name } if *name == data.table);
        if !matches {
            self.error = Some(self.localizer.text("objects-open-refused"));
        }
        matches
    }

    pub(super) fn adopt_sql_publication(&mut self) -> bool {
        self.active_object()
            .is_none_or(|tab| tab.target.kind == ObjectKind::Sql)
    }

    pub(super) fn adopt_schema_publication(&mut self, schema: &super::Schema) -> bool {
        self.active_object().is_none_or(
            |tab| matches!(&tab.target.kind, ObjectKind::Table { name } if *name == schema.table),
        )
    }

    pub(super) fn adopt_saved_inspection(&mut self, target: &super::SavedQueryTarget) -> bool {
        self.adopt_object_target(ObjectTarget {
            document_revision: target.document_revision,
            kind: ObjectKind::SavedQuery {
                object_id: target.object_id,
                name: target.name.clone(),
            },
        })
    }

    fn adopt_object_target(&mut self, target: ObjectTarget) -> bool {
        self.synchronize_objects();
        if !self.object_binding(&target) {
            self.error = Some(self.localizer.text("objects-open-refused"));
            return false;
        }
        if self.active_object().is_some_and(|tab| tab.target == target) {
            return true;
        }
        self.prepare_object_open(target).is_some()
    }

    fn park_object(&mut self) {
        let Some(id) = self.objects.active else {
            return;
        };
        let Some(kind) = self
            .objects
            .entries
            .iter()
            .find(|entry| entry.tab.id == id)
            .map(|entry| entry.tab.target.kind.clone())
        else {
            return;
        };
        let offset = self.data.as_ref().map(|data| data.offset);
        let session = Session::take(self, &kind);
        if let Some(entry) = self
            .objects
            .entries
            .iter_mut()
            .find(|entry| entry.tab.id == id)
        {
            entry.retained_bytes = session.retained_bytes();
            entry.session = Some(session);
            if let Some(offset) = offset {
                entry.offset = offset;
            }
        }
    }

    pub(super) fn refresh_parked_totals_display(&mut self) {
        for entry in &mut self.objects.entries {
            if let Some(Session::Table { totals, .. }) = &mut entry.session {
                totals.apply_display(&mut self.format_cache);
            }
            if let Some(session) = &entry.session {
                entry.retained_bytes = session.retained_bytes();
            }
        }
    }

    pub(super) fn clear_object_slots(&mut self) {
        self.table_draft = None;
        self.data = None;
        self.schema = None;
        self.query_data = None;
        self.query_result = Default::default();
        self.query_pending = None;
        self.clear_query_parameter_acquisition();
        self.close_query_action();
        self.saved_query = super::saved_query::SavedQueryState::default();
        self.table_selection = Default::default();
        self.query_selection = Default::default();
        self.table_filter = Default::default();
        self.query_design = Default::default();
        self.totals = Default::default();
        self.records = Default::default();
        self.selected_field = 0;
        self.error = None;
        self.view = DocumentView::Table;
    }

    pub(super) fn finish_object_activation(&mut self) {
        self.bump_focus_publication();
        self.design_focus_pending = None;
        self.pane_focus_entry = None;
        self.schema_activation_pending = false;
        self.ribbon_tab = if self.view == DocumentView::Design {
            super::ribbon::Tab::TableDesign
        } else {
            super::ribbon::Tab::Home
        };
        self.refresh_display();
        self.refresh_totals_display();
        if let Some(data) = &mut self.data {
            self.format_cache.apply_data(data);
        }
        if let Some(data) = &mut self.query_data {
            self.format_cache.apply_data(data);
        }
        if let Some(data) = &mut self.saved_query.data {
            self.format_cache.apply_data(data);
        }
    }

    pub(super) fn clear_object_sessions(&mut self) {
        if self.has_modified_saved_query_sql() {
            return;
        }
        self.sql.clear();
        self.objects = Objects::default();
        self.clear_object_slots();
    }
}

#[cfg(test)]
mod tests;

impl State {
    /// Explicitly invoked by a host after guarded creation, never on ordinary open.
    pub fn open_initial_table_draft(&mut self) -> bool {
        self.synchronize_objects();
        if !self.object_switch_ready() || self.database.is_none() || self.has_unsaved_table_drafts()
        {
            return false;
        }
        let Some(name) = names::suggested_table_name(self.database.as_ref().unwrap()) else {
            return false;
        };
        self.open_table_draft(super::table_draft::Editor::initial(name), false)
    }

    /// Create a manual Design draft only from the host-qualified source fact.
    pub fn open_table_design_draft(&mut self) -> bool {
        if !self.can_create_table_design() {
            return false;
        }
        self.synchronize_objects();
        let Some(name) = self.database.as_ref().and_then(names::suggested_table_name) else {
            return false;
        };
        self.open_table_draft(super::table_draft::Editor::design(name), true)
    }

    fn open_table_draft(&mut self, editor: super::table_draft::Editor, design: bool) -> bool {
        let additional = editor.retained_bytes() + size_of::<Entry>() + size_of::<Session>();
        if self
            .object_bytes()
            .and_then(|bytes| bytes.checked_add(additional))
            .is_none_or(|bytes| bytes > MAX_RETAINED_BYTES)
            || self.objects.entries.len() == MAX_OBJECTS
            || self.objects.entries.try_reserve_exact(1).is_err()
        {
            return false;
        }
        let Ok(previous) =
            NEXT_OBJECT_ID.try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        else {
            return false;
        };
        let id = previous + 1;
        let document_revision = self.database.as_ref().unwrap().document_revision;
        let tab = ObjectTab {
            id,
            target: ObjectTarget {
                document_revision,
                kind: ObjectKind::DraftTable { draft_id: id },
            },
        };
        self.park_object();
        self.clear_object_slots();
        self.table_draft = Some(editor);
        self.objects.entries.push(Entry {
            tab,
            session: None,
            offset: 0,
            retained_bytes: Some(0),
        });
        self.objects.active = Some(id);
        self.objects.navigation = None;
        self.view = if design {
            super::DocumentView::Design
        } else {
            super::DocumentView::Table
        };
        self.finish_object_activation();
        true
    }

    pub fn has_unsaved_table_drafts(&self) -> bool {
        self.objects
            .entries
            .iter()
            .any(|entry| matches!(entry.tab.target.kind, ObjectKind::DraftTable { .. }))
    }

    pub fn has_modified_table_drafts(&self) -> bool {
        self.objects
            .entries
            .iter()
            .any(|entry| self.modified_table_draft(&entry.tab))
    }

    pub(super) fn table_draft_revision_for(&self, tab: &ObjectTab) -> Option<u64> {
        if !matches!(tab.target.kind, ObjectKind::DraftTable { .. }) {
            return None;
        }
        if self.objects.active == Some(tab.id) {
            return self
                .table_draft
                .as_ref()
                .map(|editor| editor.draft.revision());
        }
        self.objects
            .entries
            .iter()
            .find(|entry| entry.tab == *tab)
            .and_then(|entry| match &entry.session {
                Some(Session::DraftTable {
                    editor: Some(editor),
                }) => Some(editor.draft.revision()),
                _ => None,
            })
    }

    pub(super) fn table_draft_property_revision_for(&self, tab: &ObjectTab) -> Option<u64> {
        self.table_draft_editor(tab)
            .map(|editor| editor.property_revision())
    }

    pub(super) fn table_draft_record_revision_for(&self, tab: &ObjectTab) -> Option<u64> {
        self.table_draft_editor(tab)
            .map(|editor| editor.record_revision())
    }

    pub(super) fn modified_table_draft(&self, tab: &ObjectTab) -> bool {
        if !matches!(tab.target.kind, ObjectKind::DraftTable { .. }) {
            return false;
        }
        if self.objects.active == Some(tab.id) {
            return self
                .table_draft
                .as_ref()
                .is_some_and(|editor| editor.is_modified());
        }
        self.objects.entries.iter().find(|entry| entry.tab == *tab).is_some_and(|entry| {
            matches!(&entry.session, Some(Session::DraftTable { editor: Some(editor) }) if editor.is_modified())
        })
    }

    pub(super) fn discard_all_table_drafts(&mut self) {
        let tabs: Vec<_> = self
            .object_tabs()
            .into_iter()
            .filter(|tab| matches!(tab.target.kind, ObjectKind::DraftTable { .. }))
            .collect();
        for tab in tabs {
            self.close_object_confirmed(&tab);
        }
    }
}

impl State {
    pub(super) fn table_draft_editor(
        &self,
        tab: &ObjectTab,
    ) -> Option<&super::table_draft::Editor> {
        let entry = self
            .objects
            .entries
            .iter()
            .find(|entry| entry.tab == *tab)?;
        if !matches!(entry.tab.target.kind, ObjectKind::DraftTable { .. }) {
            return None;
        }
        if self.objects.active == Some(tab.id) {
            return self.table_draft.as_ref();
        }
        match &entry.session {
            Some(Session::DraftTable { editor }) => editor.as_ref(),
            _ => None,
        }
    }

    pub(super) fn table_draft_editor_mut(
        &mut self,
        tab: &ObjectTab,
    ) -> Option<&mut super::table_draft::Editor> {
        let entry = self
            .objects
            .entries
            .iter_mut()
            .find(|entry| entry.tab == *tab)?;
        if !matches!(entry.tab.target.kind, ObjectKind::DraftTable { .. }) {
            return None;
        }
        if self.objects.active == Some(tab.id) {
            return self.table_draft.as_mut();
        }
        match &mut entry.session {
            Some(Session::DraftTable { editor }) => editor.as_mut(),
            _ => None,
        }
    }
}
