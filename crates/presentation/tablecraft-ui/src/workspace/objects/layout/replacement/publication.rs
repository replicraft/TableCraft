//! Validate every typed transfer slot before moving any clean SQL buffer.
use super::{
    super::{boundary, catalog},
    capture::Prepared,
};
use crate::{
    Database,
    workspace::{
        State,
        objects::{MAX_RETAINED_BYTES, ObjectKind, memory::Counter, session::Session},
        saved_query::SavedQueryState,
    },
};

pub(super) struct SqlSlot<'a> {
    source: &'a mut SavedQueryState,
    target: &'a mut SavedQueryState,
    source_error: &'a mut Option<String>,
    target_error: &'a mut Option<String>,
}

pub(in crate::workspace::objects::layout) fn publish(
    state: &mut State,
    prepared: Prepared,
    database: Database,
    change: catalog::Change<'_>,
) -> bool {
    let Prepared {
        mut objects,
        mut active,
        scratch_sql,
        scratch_revision,
        old_revision,
        transfer,
    } = prepared;
    if state.database.as_ref().is_none_or(|old| {
        old.document_revision != old_revision || !catalog::matches_change(old, &database, change)
    }) || state.has_unsaved_table_drafts()
        || state.has_modified_saved_query_sql()
        || objects.entries.len()
            != state
                .objects
                .entries
                .iter()
                .filter(|entry| transfer.retains(&entry.tab.target.kind))
                .count()
        || objects.active
            != state.objects.active.filter(|id| {
                state
                    .objects
                    .entries
                    .iter()
                    .any(|entry| entry.tab.id == *id && transfer.retains(&entry.tab.target.kind))
            })
        || !boundary::staged_optional(state, &objects, active.as_ref())
        || !objects
            .entries
            .iter()
            .zip(
                state
                    .objects
                    .entries
                    .iter()
                    .filter(|entry| transfer.retains(&entry.tab.target.kind)),
            )
            .all(|(new, old)| {
                new.tab.id == old.tab.id
                    && new.tab.target.kind == old.tab.target.kind
                    && new.tab.target.document_revision == database.document_revision
                    && old.tab.target.document_revision == old_revision
            })
    {
        return false;
    }
    let active_id = objects.active;
    let mut slots = Vec::new();
    if slots.try_reserve_exact(objects.entries.len()).is_err() {
        return false;
    }
    let Some(current_bytes) = state.object_bytes() else {
        return false;
    };
    let Some(old_catalog) = state.database.as_ref().and_then(catalog::retained_bytes) else {
        return false;
    };
    let Some(fresh_catalog) = catalog::retained_bytes(&database) else {
        return false;
    };
    let Some(object_bytes) = objects.retained_bytes() else {
        return false;
    };
    let Some(active_bytes) = active.as_ref().map_or(Some(0), Session::retained_bytes) else {
        return false;
    };
    let mut transient = Counter::new(current_bytes);
    transient.add(old_catalog);
    transient.add(fresh_catalog);
    transient.add(object_bytes);
    transient.add(active_bytes);
    transient.string(&scratch_sql);
    transient.vector(&slots);
    if transient
        .finish()
        .is_none_or(|bytes| bytes > MAX_RETAINED_BYTES)
    {
        return false;
    }

    let mut old_active = Some((&mut state.saved_query, &mut state.error));
    let mut new_active = match active.as_mut() {
        Some(Session::Saved {
            state: saved,
            error,
        }) => Some((saved.as_mut(), error)),
        _ => None,
    };
    for (old, new) in state
        .objects
        .entries
        .iter_mut()
        .filter(|entry| transfer.retains(&entry.tab.target.kind))
        .zip(&mut objects.entries)
    {
        if !matches!(old.tab.target.kind, ObjectKind::SavedQuery { .. }) {
            continue;
        }
        let base_bytes = if Some(new.tab.id) == active_id {
            Some(0)
        } else {
            new.session.as_ref().and_then(Session::retained_bytes)
        };
        let (source, source_error) = if Some(old.tab.id) == active_id {
            let Some(owner) = old_active.take() else {
                return false;
            };
            owner
        } else {
            let Some(Session::Saved {
                state: saved,
                error,
            }) = &mut old.session
            else {
                return false;
            };
            (saved.as_mut(), error)
        };
        let (target, target_error) = if Some(new.tab.id) == active_id {
            let Some(owner) = new_active.take() else {
                return false;
            };
            owner
        } else {
            let Some(Session::Saved {
                state: saved,
                error,
            }) = &mut new.session
            else {
                return false;
            };
            (saved.as_mut(), error)
        };
        if Some(new.tab.id) != active_id {
            let Some(bytes) = base_bytes.and_then(|base| {
                base.checked_add(source.moved_sql_payload_bytes(None)?)?
                    .checked_add(source_error.as_ref().map_or(0, String::capacity))
            }) else {
                return false;
            };
            new.retained_bytes = Some(bytes);
        }
        slots.push(SqlSlot {
            source,
            target,
            source_error,
            target_error,
        });
    }

    // Commit point: all identities, graph slots and allocations were validated.
    // No fallible operation, allocation or lookup follows the first owned take.
    for slot in slots {
        slot.source
            .move_sql_rebind_to(slot.target, database.document_revision, None);
        *slot.target_error = slot.source_error.take();
    }
    state.database = Some(database);
    state.objects = objects;
    state.sql = scratch_sql;
    state.query_sql_revision = scratch_revision;
    state.table_creation = None;
    state.table_design = None;
    if let Some(active) = active {
        active.install(state);
    } else {
        state.clear_object_slots();
    }
    state.bump_focus_publication();
    state.schema_activation_pending = false;
    state.ribbon_tab = if state.view == crate::workspace::DocumentView::Design {
        crate::workspace::ribbon::Tab::TableDesign
    } else {
        crate::workspace::ribbon::Tab::Home
    };
    true
}
