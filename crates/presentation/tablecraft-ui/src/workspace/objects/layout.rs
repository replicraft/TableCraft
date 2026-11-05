//! Source-free workspace preferences cannot restore old row or tab authority.
use super::session::Entry;
use super::{MAX_OBJECTS, NEXT_OBJECT_ID, ObjectKind, ObjectTab, ObjectTarget, Objects, Session};
use crate::SavedQueryTarget;
use crate::workspace::query_design::{Preferences as QueryDesignPreferences, QueryDesign};
use crate::workspace::saved_query::InputPreferences;
use crate::workspace::totals::{Totals, TotalsPreferences};
use crate::workspace::{DocumentView, State};
use std::sync::atomic::Ordering;

pub struct WorkspaceLayout {
    entries: Vec<Preference>,
    active: Option<ObjectKind>,
    scratch_sql: String,
}

struct Preference {
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
                        Some(Session::Sql { .. }) => (false, 0, None),
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
                    query_design,
                    totals,
                    kind: entry.tab.target.kind.clone(),
                    design,
                    offset,
                    parameters,
                }
            })
            .collect();
        WorkspaceLayout {
            entries,
            active: self.active_object().map(|tab| tab.target.kind),
            scratch_sql: self.sql.clone(),
        }
    }

    pub fn restore_workspace_layout(
        &mut self,
        layout: WorkspaceLayout,
        preferred: ObjectKind,
    ) -> bool {
        let Some(revision) = self
            .database
            .as_ref()
            .map(|database| database.document_revision)
        else {
            return false;
        };
        if !self.object_switch_ready()
            || layout.entries.len() > MAX_OBJECTS
            || layout.entries.iter().any(|entry| {
                !self.object_binding(&ObjectTarget {
                    document_revision: revision,
                    kind: entry.kind.clone(),
                })
            })
        {
            self.error = Some(self.localizer.text("objects-restore-refused"));
            return false;
        }
        let mut restored = Objects {
            revision: Some(revision),
            ..Objects::default()
        };
        if restored
            .entries
            .try_reserve_exact(layout.entries.len())
            .is_err()
        {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return false;
        }
        for preference in layout.entries {
            let Ok(previous) =
                NEXT_OBJECT_ID
                    .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            else {
                self.error = Some(self.localizer.text("objects-restore-refused"));
                return false;
            };
            let target = ObjectTarget {
                document_revision: revision,
                kind: preference.kind,
            };
            let mut session = Session::empty(&target.kind);
            match &mut session {
                Session::Table { design, totals, .. } => {
                    *design = preference.design;
                    *totals = Totals::restore(preference.totals);
                }
                Session::Saved { state, .. } => {
                    let ObjectKind::SavedQuery { object_id, name } = &target.kind else {
                        unreachable!();
                    };
                    state.restore_preferences(
                        SavedQueryTarget {
                            document_revision: revision,
                            object_id: *object_id,
                            name: name.clone(),
                        },
                        preference.parameters,
                    );
                }
                Session::Sql { design, .. } => {
                    *design = preference
                        .query_design
                        .map(QueryDesign::from_preferences)
                        .unwrap_or_default();
                }
            }
            let retained_bytes = session.retained_bytes();
            restored.entries.push(Entry {
                tab: ObjectTab {
                    id: previous + 1,
                    target,
                },
                session: Some(session),
                offset: preference.offset,
                retained_bytes,
            });
        }
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
        self.clear_object_slots();
        self.objects = restored;
        self.sql = layout.scratch_sql;
        if let Some(active) = active {
            return self.activate_object(&active);
        }
        true
    }
}
