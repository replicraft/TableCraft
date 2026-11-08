//! Stage only object preferences and empty sessions, retaining original tab IDs.
use super::super::{catalog, restoration};
use crate::{
    Database,
    workspace::{
        State,
        objects::{MAX_RETAINED_BYTES, ObjectKind, ObjectTarget, Objects, session::Session},
        saved_query::copy_rebind_text,
    },
};

pub(super) struct Prepared {
    pub(super) objects: Objects,
    pub(super) active: Session,
    pub(super) scratch_sql: String,
    pub(super) scratch_revision: u64,
    pub(super) old_revision: u64,
}

fn navigation(old: &ObjectTarget, revision: u64) -> Option<ObjectTarget> {
    let kind = match &old.kind {
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
    Some(ObjectTarget {
        document_revision: revision,
        kind,
    })
}

pub(super) fn prepare(state: &mut State, fresh: &Database) -> Option<Prepared> {
    let active_id = state.objects.active?;
    let old_revision = state.database.as_ref()?.document_revision;
    let scratch_revision = state.query_sql_revision.checked_add(1)?;
    // Structural preflight proves synchronize_objects is a no-op here. Capture
    // copies only preferences/scratch text, never native rows or query results.
    let layout = state.capture_workspace_layout();
    let mut objects = restoration::prepare(
        layout.entries,
        fresh.document_revision,
        Some(&state.objects),
    )
    .ok()?;
    drop(layout.active);
    objects.active = Some(active_id);
    objects.navigation = match &state.objects.navigation {
        Some(old) => Some(navigation(old, fresh.document_revision)?),
        None => None,
    };
    let current = state
        .object_bytes()?
        .checked_add(catalog::retained_bytes(state.database.as_ref()?)?)?;
    let prepared = objects
        .retained_bytes()?
        .checked_add(layout.scratch_sql.capacity())?
        .checked_add(catalog::retained_bytes(fresh)?)?;
    if current.checked_add(prepared)? > MAX_RETAINED_BYTES {
        return None;
    }
    let entry = objects
        .entries
        .iter_mut()
        .find(|entry| entry.tab.id == active_id)?;
    let active = entry.session.take()?;
    entry.retained_bytes = Some(0);
    Some(Prepared {
        objects,
        active,
        scratch_sql: layout.scratch_sql,
        scratch_revision,
        old_revision,
    })
}
