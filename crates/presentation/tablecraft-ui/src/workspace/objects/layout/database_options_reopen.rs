//! Database-options publication accepts only its distinct full-image native witness.
use super::{catalog, replacement};
use crate::{
    Database,
    workspace::{
        DatabaseOptionsSaveIdentity, State,
        objects::{MAX_OBJECTS, MAX_RETAINED_BYTES},
    },
};

impl State {
    /// Structural preflight, allowing this operation's own busy/modal status.
    /// It neither qualifies database settings nor infers native write authority.
    pub fn can_rebind_database_options_layout(&self, revision: u64) -> bool {
        self.database
            .as_ref()
            .is_some_and(|database| database.document_revision == revision)
            && self.objects.revision == Some(revision)
            && self.objects.entries.len() <= MAX_OBJECTS
            && self.objects.active.is_none_or(|active| {
                self.objects
                    .entries
                    .iter()
                    .any(|entry| entry.tab.id == active && entry.session.is_none())
            })
            && !self.field_addition_open()
            && !self.field_addition_source_stale()
            && replacement::ready(self)
            && self
                .object_bytes()
                .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES)
            && self
                .database
                .as_ref()
                .is_some_and(|database| replacement::fits(self, database) == Some(true))
    }

    /// Host preconditions: the exact options actor acknowledged known AFTER and
    /// OpenDatabaseOptionsSaved successfully verified its full expected image.
    /// That operation proves all existing user object definitions unchanged.
    /// Database metadata alone cannot establish this authority, and a query or
    /// column witness cannot substitute for DatabaseOptionsReopen.
    ///
    /// Close only the explicitly confirmed persisted object tabs; retain source-free
    /// scratch editor IDs/text/preferences as a TableCraft tool boundary. This
    /// is not evidence that Access retains an unsaved native Query object.
    /// Invalidate old native results/handles. Keep options status until the host
    /// completes publication and clears that exact owner. Refusal changes no text.
    pub fn resolve_database_options_after_reopen(
        &mut self,
        identity: DatabaseOptionsSaveIdentity,
        database: Database,
    ) -> bool {
        if !self.database_options_close_graph_current()
            || !self.database_options_reopen_pending(identity)
            || !self.can_rebind_database_options_layout(identity.document_revision)
            || self
                .database
                .as_ref()
                .is_none_or(|old| !catalog::matches(old, &database))
            || replacement::fits(self, &database) != Some(true)
        {
            return false;
        }
        let Some(prepared) =
            replacement::prepare(self, &database, replacement::Transfer::ClosePersisted)
        else {
            return false;
        };
        replacement::publish(self, prepared, database)
    }
}

impl State {
    pub(in crate::workspace) fn capture_database_options_close_graph(
        &mut self,
        confirmation_id: u64,
    ) -> Option<crate::workspace::database_options::CloseGraph> {
        use crate::workspace::{
            ObjectKind, ObjectTab, ObjectTarget,
            database_options::{CloseGraph, DatabaseOptionsCloseRequest},
            saved_query::copy_rebind_text,
        };
        let revision = self.database.as_ref()?.document_revision;
        let additional = self.objects.entries.iter().try_fold(
            self.objects
                .entries
                .len()
                .checked_mul(size_of::<ObjectTab>())?,
            |bytes, entry| {
                bytes.checked_add(match &entry.tab.target.kind {
                    ObjectKind::Table { name } | ObjectKind::SavedQuery { name, .. } => name.len(),
                    ObjectKind::Sql => 0,
                    ObjectKind::DraftTable { .. } => return None,
                })
            },
        );
        if !self.admit_object_bytes(additional) {
            return None;
        }
        let mut tabs = Vec::new();
        tabs.try_reserve_exact(self.objects.entries.len()).ok()?;
        for entry in &self.objects.entries {
            let kind = match &entry.tab.target.kind {
                ObjectKind::Table { name } => ObjectKind::Table {
                    name: copy_rebind_text(name)?,
                },
                ObjectKind::SavedQuery { object_id, name } => ObjectKind::SavedQuery {
                    object_id: *object_id,
                    name: copy_rebind_text(name)?,
                },
                ObjectKind::Sql => ObjectKind::Sql,
                ObjectKind::DraftTable { .. } => return None,
            };
            tabs.push(ObjectTab {
                id: entry.tab.id,
                target: ObjectTarget {
                    document_revision: entry.tab.target.document_revision,
                    kind,
                },
            });
        }
        let graph = CloseGraph {
            request: DatabaseOptionsCloseRequest {
                document_revision: revision,
                confirmation_id,
            },
            tabs,
            active: self.objects.active,
            approved: false,
        };
        if !self.admit_object_bytes(graph.heap_bytes()) {
            return None;
        }
        Some(graph)
    }

    pub(in crate::workspace) fn database_options_graph_matches(
        &self,
        graph: &crate::workspace::database_options::CloseGraph,
    ) -> bool {
        self.database
            .as_ref()
            .is_some_and(|database| database.document_revision == graph.request.document_revision)
            && self.objects.active == graph.active
            && self.objects.entries.len() == graph.tabs.len()
            && self
                .objects
                .entries
                .iter()
                .zip(&graph.tabs)
                .all(|(entry, captured)| entry.tab == *captured)
    }
}
