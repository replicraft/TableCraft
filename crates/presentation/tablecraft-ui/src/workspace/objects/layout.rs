//! Source-free workspace preferences cannot restore old row or tab authority.
mod boundary;
mod catalog;
mod database_options_reopen;
mod field_addition_reopen;
mod field_drop_reopen;
mod field_rename_reopen;
mod query_schema_reopen;
mod replacement;
mod restoration;
mod saved_sql_reopen;
mod table_drop_reopen;

use super::{MAX_OBJECTS, ObjectKind, ObjectTarget, Session};
use crate::workspace::query_design::Preferences as QueryDesignPreferences;
use crate::workspace::saved_query::InputPreferences;
use crate::workspace::totals::TotalsPreferences;
use crate::workspace::{DocumentView, State};

pub struct WorkspaceLayout {
    entries: Vec<Preference>,
    revision: Option<u64>,
    active: Option<ObjectKind>,
    scratch_sql: String,
}

struct Preference {
    id: u64,
    kind: ObjectKind,
    design: bool,
    offset: u32,
    parameters: Option<InputPreferences>,
    totals: Option<TotalsPreferences>,
    query_design: Option<QueryDesignPreferences>,
}

impl State {
    pub fn capture_workspace_layout(&mut self) -> WorkspaceLayout {
        self.synchronize_objects();
        let entries = self
            .objects
            .entries
            .iter()
            .filter(|entry| !matches!(entry.tab.target.kind, ObjectKind::DraftTable { .. }))
            .map(|entry| {
                let active = self.objects.active == Some(entry.tab.id);
                let (design, offset, parameters) = if active {
                    (
                        self.view == DocumentView::Design,
                        self.data.as_ref().map_or(entry.offset, |data| data.offset),
                        if matches!(entry.tab.target.kind, ObjectKind::SavedQuery { .. }) {
                            self.saved_query.input_preferences()
                        } else {
                            None
                        },
                    )
                } else {
                    match &entry.session {
                        Some(Session::Table { data, design, .. }) => (
                            *design,
                            data.as_ref().map_or(entry.offset, |data| data.offset),
                            None,
                        ),
                        Some(Session::Saved { state, .. }) => (false, 0, state.input_preferences()),
                        Some(Session::DraftTable { .. } | Session::Sql { .. }) => (false, 0, None),
                        None => (false, entry.offset, None),
                    }
                };
                let totals = if active {
                    self.totals.preferences()
                } else {
                    match &entry.session {
                        Some(Session::Table { totals, .. }) => totals.preferences(),
                        _ => None,
                    }
                };
                let query_design = if active {
                    self.query_design.preferences()
                } else {
                    match &entry.session {
                        Some(Session::Sql { design, .. }) => design.preferences(),
                        _ => None,
                    }
                };
                Preference {
                    id: entry.tab.id,
                    query_design,
                    totals,
                    // NO_CLONE: Capture a distinct restoration/edit-input snapshot while the
                    // original live declarations, draft, tab or totals preferences
                    // remain in use; parameter edit text and declaration identity are
                    // separate owners.
                    kind: entry.tab.target.kind.clone(),
                    design,
                    offset,
                    parameters,
                }
            })
            .collect();
        WorkspaceLayout {
            entries,
            revision: self.objects.revision,
            active: self.active_object().and_then(|tab| {
                (!matches!(tab.target.kind, ObjectKind::DraftTable { .. }))
                    .then_some(tab.target.kind)
            }),
            // NO_CLONE: Capture a distinct restoration/edit-input snapshot while the original
            // live declarations, draft, tab or totals preferences remain in use;
            // parameter edit text and declaration identity are separate owners.
            scratch_sql: self.sql.clone(),
        }
    }

    pub fn restore_workspace_layout(
        &mut self,
        layout: WorkspaceLayout,
        preferred: ObjectKind,
    ) -> bool {
        self.restore_workspace_layout_with_identity(layout, preferred, false)
    }

    /// Restore presentation identities after the host has saved and reopened
    /// the same document. Every source-bound session is reconstructed empty at
    /// the fresh revision; retaining a UI ID grants no old row or query rights.
    ///
    /// The host must correlate the completed save and fresh open before calling
    /// this method. Ordinary cross-document preference restore uses fresh IDs.
    pub fn restore_workspace_layout_after_save(
        &mut self,
        layout: WorkspaceLayout,
        preferred: ObjectKind,
    ) -> bool {
        self.restore_workspace_layout_with_identity(layout, preferred, true)
    }

    fn restore_workspace_layout_with_identity(
        &mut self,
        layout: WorkspaceLayout,
        preferred: ObjectKind,
        retain_ids: bool,
    ) -> bool {
        let Some(revision) = self
            .database
            .as_ref()
            .map(|database| database.document_revision)
        else {
            return false;
        };
        if (retain_ids && layout.revision.is_none_or(|old| old == revision))
            || !self.object_switch_ready()
            || self.has_unsaved_table_drafts()
            || layout.entries.len() > MAX_OBJECTS
            || layout.entries.iter().any(|entry| {
                !self.object_binding(&ObjectTarget {
                    document_revision: revision,
                    // NO_CLONE: The restore or shortcut candidate retains its exact identity
                    // independently of the original layout/registry and active-owner
                    // witness before registry installation or deferred dispatch.
                    kind: entry.kind.clone(),
                })
            })
        {
            self.error = Some(self.localizer.text("objects-restore-refused"));
            return false;
        }
        let prepared = if retain_ids {
            restoration::prepare_after_save(layout.entries, revision)
        } else {
            restoration::prepare(layout.entries, revision, None)
        };
        let restored = match prepared {
            Ok(objects) => objects,
            Err(restoration::PrepareError::Allocation) => {
                self.error = Some(self.localizer.text("objects-memory-limit"));
                return false;
            }
            Err(restoration::PrepareError::Identity) => {
                self.error = Some(self.localizer.text("objects-restore-refused"));
                return false;
            }
        };
        // NO_CLONE: The restore or shortcut candidate retains its exact identity independently
        // of the original layout/registry and active-owner witness before registry
        // installation or deferred dispatch.
        let active = restored
            .entries
            .iter()
            .find(|entry| entry.tab.target.kind == preferred)
            .or_else(|| {
                restored
                    .entries
                    .iter()
                    .find(|entry| Some(&entry.tab.target.kind) == layout.active.as_ref())
            })
            .or_else(|| restored.entries.first())
            .map(|entry| entry.tab.clone());
        if restored
            .retained_bytes()
            .and_then(|bytes| bytes.checked_add(layout.scratch_sql.capacity()))
            .is_none_or(|bytes| bytes > super::MAX_RETAINED_BYTES)
        {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return false;
        }
        if !self.replace_query_sql(layout.scratch_sql) {
            self.error = Some(self.localizer.text("objects-open-refused"));
            return false;
        }
        self.clear_object_slots();
        self.objects = restored;
        if let Some(active) = active {
            return self.activate_object(&active);
        }
        true
    }
}
