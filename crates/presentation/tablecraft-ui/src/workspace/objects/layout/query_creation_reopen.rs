//! Publish the actual created query while retaining every preexisting authoring owner.
use super::{Preference, boundary, catalog, restoration};
use crate::workspace::{
    State,
    objects::{
        MAX_OBJECTS, MAX_RETAINED_BYTES, NEXT_OBJECT_ID, ObjectKind, ObjectTarget, Objects,
        memory::Counter,
        session::{Entry, Session},
    },
    query::parameters::copy_text,
    query_design::QueryDesign,
    saved_query::SavedQueryState,
};
use crate::{Database, QueryCreationRequest, QueryCreationSaveIdentity, SavedQueryTarget};
use std::sync::atomic::Ordering;

struct SavedSlot<'a> {
    source: &'a mut SavedQueryState,
    target: &'a mut SavedQueryState,
    source_error: &'a mut Option<String>,
    target_error: &'a mut Option<String>,
}

fn design(state: &State, id: u64) -> Option<&QueryDesign> {
    if state.objects.active == Some(id) {
        return Some(&state.query_design);
    }
    match state
        .objects
        .entries
        .iter()
        .find(|entry| entry.tab.id == id)?
        .session
        .as_ref()?
    {
        Session::Sql { design, .. } => Some(design),
        Session::Saved { state, .. } => Some(&state.design),
        _ => None,
    }
}

impl State {
    /// Source-free structural preflight; it predicts no native eligibility or receipt.
    /// It deliberately permits the scoped creation's own pending/status owners.
    pub fn can_rebind_query_creation_layout(&self, request: &QueryCreationRequest) -> bool {
        let Some(database) = &self.database else {
            return false;
        };
        let count = self.objects.entries.len();
        let extra = usize::from(request.source.is_some());
        request.document_revision == database.document_revision
            && self.objects.revision == Some(database.document_revision)
            && self.objects.active.is_some()
            && count > 0
            && count
                .checked_add(extra)
                .is_some_and(|count| count <= MAX_OBJECTS)
            && self
                .objects
                .entries
                .iter()
                .filter(|entry| entry.tab == request.tab)
                .count()
                == 1
            && boundary::current(self)
            && self
                .objects
                .entries
                .iter()
                .all(|entry| boundary::bound(database, &entry.tab.target))
            && !self.has_unsaved_table_drafts()
            && self.table_draft.is_none()
            && !self.saved_query_sql_close_open()
            && self.saved_query_sql_save_status().is_none()
            && !self.field_rename_open()
            && !self.field_rename_status_open()
            && !self.field_addition_close_open()
            && self.table_draft_close.is_none()
            && !self.edit_dialog_open()
            && !self.insertion_dialog_open()
            && !self.deletion_dialog_open()
            && !self.recovery_prompt_open()
            && self.contents_request_id().is_none()
            && !self.parameter_prompt_open()
            && !self.query_action_dialog_open()
            && !self.table_draft_save_open()
            && !self.table_draft_field_delete_open()
            && self.query_sql_revision.checked_add(1).is_some()
            && request.text_revision.checked_add(1).is_some()
            && self
                .object_bytes()
                .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES)
    }

    /// Caller must supply the acknowledged nominal Created outcome and successful
    /// complete-image native reopen. Catalog metadata alone grants no authority.
    /// A refusal leaves the old catalog and all authoring owners untouched.
    pub fn resolve_query_creation_reopen(
        &mut self,
        identity: QueryCreationSaveIdentity,
        fresh: Database,
        created: SavedQueryTarget,
    ) -> bool {
        let Some(request) = self.query_creation_reopen_request(identity) else {
            return false;
        };
        if !self.can_rebind_query_creation_layout(&request)
            || created.document_revision != fresh.document_revision
            || created.name != request.destination
            || !self.database.as_ref().is_some_and(|old| {
                catalog::matches_change(
                    old,
                    &fresh,
                    catalog::Change::QueryCreated {
                        object_id: created.object_id,
                        name: &created.name,
                    },
                )
            })
            || !fits(self, &request, &fresh).is_some_and(|fits| fits)
        {
            return false;
        }
        let Some(prepared) = prepare(self, &request, &fresh, created) else {
            return false;
        };
        publish(self, prepared, fresh)
    }
}

fn fits(state: &State, request: &QueryCreationRequest, fresh: &Database) -> Option<bool> {
    let mut count = Counter::new(state.object_bytes()?);
    count.add(catalog::retained_bytes(state.database.as_ref()?)?);
    count.add(catalog::retained_bytes(fresh)?);
    count.add(size_of::<Objects>());
    count.count::<Entry>(
        state
            .objects
            .entries
            .len()
            .checked_add(usize::from(request.source.is_some()))?,
    );
    count.count::<Preference>(MAX_OBJECTS);
    count.count::<SavedSlot<'static>>(state.objects.entries.len());
    if let Some(active) = state.objects.active.and_then(|id| {
        state
            .objects
            .entries
            .iter()
            .find(|entry| entry.tab.id == id)
    }) {
        match &active.tab.target.kind {
            ObjectKind::Table { name } | ObjectKind::SavedQuery { name, .. } => count.string(name),
            ObjectKind::Sql => {}
            ObjectKind::DraftTable { .. } => return Some(false),
        }
    }
    count.string(&state.sql);
    count.count::<SavedQueryState>(1);
    // New live target, independent SQL-draft target, preference and navigation
    // own separate spellings while the request remains retained by the status.
    for _ in 0..4 {
        count.string(&request.destination);
    }
    // Stored baseline and editable authored text are distinct mutable owners.
    count.string(request.sql.as_ref());
    count.string(request.sql.as_ref());
    if let Some(navigation) = &state.objects.navigation {
        match &navigation.kind {
            ObjectKind::Table { name } | ObjectKind::SavedQuery { name, .. } => count.string(name),
            ObjectKind::Sql => {}
            ObjectKind::DraftTable { .. } => return Some(false),
        }
    }
    for entry in &state.objects.entries {
        let empty = Session::empty(&entry.tab.target.kind);
        count.add(empty.retained_bytes()?);
        match &entry.tab.target.kind {
            ObjectKind::Table { name } => {
                count.string(name);
                let totals = if state.objects.active == Some(entry.tab.id) {
                    &state.totals
                } else {
                    let Some(Session::Table { totals, .. }) = &entry.session else {
                        return Some(false);
                    };
                    totals
                };
                count.add(totals.rebind_preference_bytes()?);
            }
            ObjectKind::SavedQuery { name, .. } => {
                count.string(name);
                count.string(name);
                let owner = if state.objects.active == Some(entry.tab.id) {
                    &state.saved_query
                } else {
                    let Some(Session::Saved { state, .. }) = &entry.session else {
                        return Some(false);
                    };
                    state.as_ref()
                };
                count.add(owner.sql_preference_copy_bytes()?);
                count.add(design(state, entry.tab.id)?.rebind_preference_bytes()?);
            }
            ObjectKind::Sql => count.add(design(state, entry.tab.id)?.rebind_preference_bytes()?),
            ObjectKind::DraftTable { .. } => return Some(false),
        }
    }
    if request.source.is_some() {
        count.add(design(state, request.tab.id)?.rebind_preference_bytes()?);
    }
    Some(count.finish()? <= MAX_RETAINED_BYTES)
}

struct Prepared {
    objects: Objects,
    active: Session,
    scratch_sql: String,
    scratch_revision: u64,
    old_revision: u64,
    old_count: usize,
}

fn prepare(
    state: &mut State,
    request: &QueryCreationRequest,
    fresh: &Database,
    created: SavedQueryTarget,
) -> Option<Prepared> {
    let saved_copy = request.source.is_some();
    let extra_design = saved_copy
        .then(|| design(state, request.tab.id)?.preferences())
        .flatten();
    let old_count = state.objects.entries.len();
    let mut layout = state.capture_workspace_layout();
    let source = layout
        .entries
        .iter()
        .position(|entry| entry.id == request.tab.id)?;
    // NO_CLONE: The restored tab owns a name independently of the new live
    // saved-query target passed to its typed state constructor below.
    let created_kind = ObjectKind::SavedQuery {
        object_id: created.object_id,
        name: copy_text(&created.name)?,
    };
    let created_id = if saved_copy {
        let id = NEXT_OBJECT_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .ok()?
            .checked_add(1)?;
        layout.entries.try_reserve_exact(1).ok()?;
        layout.entries.push(Preference {
            id,
            kind: created_kind,
            design: false,
            offset: 0,
            parameters: None,
            totals: None,
            query_design: extra_design,
        });
        id
    } else {
        let preference = &mut layout.entries[source];
        preference.kind = created_kind;
        preference.parameters = None;
        preference.totals = None;
        preference.offset = 0;
        request.tab.id
    };
    let mut objects =
        restoration::prepare_after_save(layout.entries, fresh.document_revision).ok()?;
    objects.active = Some(created_id);
    // NO_CLONE: Navigation and the tab registry are independent presentation
    // owners, both bound to the same actual fresh created identity.
    objects.navigation = Some(ObjectTarget {
        document_revision: fresh.document_revision,
        kind: ObjectKind::SavedQuery {
            object_id: created.object_id,
            name: copy_text(&created.name)?,
        },
    });
    let entry = objects
        .entries
        .iter_mut()
        .find(|entry| entry.tab.id == created_id)?;
    let Session::Saved {
        state: restored, ..
    } = entry.session.as_mut()?
    else {
        return None;
    };
    let mut saved =
        SavedQueryState::created(created, &request.sql, request.text_revision.checked_add(1)?)?;
    saved.design = std::mem::take(&mut restored.design);
    **restored = saved;
    let active = entry.session.take()?;
    entry.retained_bytes = Some(0);
    let bytes = state
        .object_bytes()?
        .checked_add(catalog::retained_bytes(state.database.as_ref()?)?)?
        .checked_add(catalog::retained_bytes(fresh)?)?
        .checked_add(objects.retained_bytes()?)?
        .checked_add(active.retained_bytes()?)?
        .checked_add(layout.scratch_sql.capacity())?;
    if bytes > MAX_RETAINED_BYTES {
        return None;
    }
    Some(Prepared {
        objects,
        active,
        scratch_sql: layout.scratch_sql,
        scratch_revision: state.query_sql_revision.checked_add(1)?,
        old_revision: request.document_revision,
        old_count,
    })
}

fn publish(state: &mut State, prepared: Prepared, fresh: Database) -> bool {
    let Prepared {
        mut objects,
        active,
        scratch_sql,
        scratch_revision,
        old_revision,
        old_count,
    } = prepared;
    if state
        .database
        .as_ref()
        .is_none_or(|old| old.document_revision != old_revision)
        || state.objects.entries.len() != old_count
        || !boundary::current(state)
        || !objects
            .entries
            .iter()
            .all(|entry| boundary::bound(&fresh, &entry.tab.target))
    {
        return false;
    }
    let old_active_id = state.objects.active;
    let mut slots = Vec::new();
    if slots.try_reserve_exact(old_count).is_err() {
        return false;
    }
    let Some(current_bytes) = state.object_bytes() else {
        return false;
    };
    let mut count = Counter::new(current_bytes);
    let Some(old_catalog) = state.database.as_ref().and_then(catalog::retained_bytes) else {
        return false;
    };
    let Some(fresh_catalog) = catalog::retained_bytes(&fresh) else {
        return false;
    };
    let Some(objects_bytes) = objects.retained_bytes() else {
        return false;
    };
    let Some(active_bytes) = active.retained_bytes() else {
        return false;
    };
    count.add(old_catalog);
    count.add(fresh_catalog);
    count.add(objects_bytes);
    count.add(active_bytes);
    count.string(&scratch_sql);
    count.vector(&slots);
    if count
        .finish()
        .is_none_or(|bytes| bytes > MAX_RETAINED_BYTES)
    {
        return false;
    }
    let mut live = Some((&mut state.saved_query, &mut state.error));
    for (old, new) in state.objects.entries.iter_mut().zip(&mut objects.entries) {
        if old.tab.id != new.tab.id || old.tab.target.document_revision != old_revision {
            return false;
        }
        if !matches!(old.tab.target.kind, ObjectKind::SavedQuery { .. }) {
            continue;
        }
        if old.tab.target.kind != new.tab.target.kind {
            return false;
        }
        let base = new.session.as_ref().and_then(Session::retained_bytes);
        let (source, source_error) = if old_active_id == Some(old.tab.id) {
            let Some(source) = live.take() else {
                return false;
            };
            source
        } else {
            let Some(Session::Saved { state, error }) = &mut old.session else {
                return false;
            };
            (state.as_mut(), error)
        };
        let Some(Session::Saved {
            state: target,
            error: target_error,
        }) = &mut new.session
        else {
            return false;
        };
        let Some(bytes) = base.and_then(|bytes| {
            bytes
                .checked_add(source.moved_sql_payload_bytes(None)?)?
                .checked_add(source_error.as_ref().map_or(0, String::capacity))
        }) else {
            return false;
        };
        new.retained_bytes = Some(bytes);
        slots.push(SavedSlot {
            source,
            target: target.as_mut(),
            source_error,
            target_error,
        });
    }
    // Publication point: every source/destination slot, allocation and identity
    // has been validated. Existing raw SQL/error buffers move without cloning.
    for slot in slots {
        slot.source
            .move_sql_rebind_to(slot.target, fresh.document_revision, None);
        *slot.target_error = slot.source_error.take();
    }
    state.database = Some(fresh);
    state.objects = objects;
    state.sql = scratch_sql;
    state.query_sql_revision = scratch_revision;
    state.table_creation = None;
    state.table_design = None;
    state.saved_query_sql_close = None;
    active.install(state);
    state.bump_focus_publication();
    state.schema_activation_pending = false;
    state.ribbon_tab = if state.query_design_visible() {
        crate::workspace::ribbon::Tab::QueryDesign
    } else {
        crate::workspace::ribbon::Tab::Home
    };
    true
}
