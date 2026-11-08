//! Prepare all owned metadata, then collect typed transfer slots before any take.
use super::{super::restoration, catalog};
use crate::workspace::{
    State,
    objects::{MAX_RETAINED_BYTES, ObjectKind, ObjectTarget, Objects, session::Session},
    saved_query::{SqlResolution, copy_rebind_text},
};
use crate::{Database, SavedQuerySqlRequest};

pub(super) struct Prepared {
    pub(super) objects: Objects,
    pub(super) active: Session,
    pub(super) scratch_sql: String,
    pub(super) scratch_revision: u64,
    pub(super) old_revision: u64,
    pub(super) target_id: u64,
    pub(super) resolution: SqlResolution,
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

pub(super) fn prepare(
    state: &mut State,
    request: &SavedQuerySqlRequest,
    fresh: &Database,
) -> Option<Prepared> {
    let active_id = state.objects.active?;
    let target_id = state
        .objects
        .entries
        .iter()
        .find(|entry| {
            matches!(&entry.tab.target.kind, ObjectKind::SavedQuery {object_id, name}
            if *object_id == request.target.object_id && *name == request.target.name)
        })?
        .tab
        .id;
    let resolution = SqlResolution::prepare(request)?;
    // The initialized revision/active-owner preflight makes synchronize_objects
    // a no-op. Existing owner capture copies preferences, never native rows.
    let layout = state.capture_workspace_layout();
    let scratch_revision = state.query_sql_revision.checked_add(1)?;
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
    let current_bytes = state
        .object_bytes()?
        .checked_add(catalog::retained_bytes(state.database.as_ref()?)?)?;
    let proposed_bytes = objects
        .retained_bytes()?
        .checked_add(layout.scratch_sql.capacity())?
        .checked_add(catalog::retained_bytes(fresh)?)?
        .checked_add(resolution.retained_bytes())?;
    if current_bytes.checked_add(proposed_bytes)? > MAX_RETAINED_BYTES {
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
        old_revision: request.target.document_revision,
        target_id,
        resolution,
    })
}
