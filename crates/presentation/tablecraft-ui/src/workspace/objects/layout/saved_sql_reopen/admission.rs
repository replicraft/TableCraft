//! Capacity bounds cover actual transferred carriers, not obsolete row caches.
use super::super::Preference;
use super::super::catalog;
use crate::workspace::{
    State,
    objects::{
        MAX_OBJECTS, MAX_RETAINED_BYTES, ObjectKind, Objects,
        memory::Counter,
        session::{Entry, Session},
    },
};
use crate::{Database, SavedQuerySqlRequest};

pub(super) fn fits(
    state: &State,
    request: &SavedQuerySqlRequest,
    fresh: &Database,
) -> Option<bool> {
    let mut count = Counter::new(state.object_bytes()?);
    count.add(catalog::retained_bytes(state.database.as_ref()?)?);
    count.add(catalog::retained_bytes(fresh)?);
    count.add(size_of::<Objects>());
    count.count::<Entry>(state.objects.entries.len());
    count.count::<Preference>(MAX_OBJECTS);
    count.count::<super::publication::SqlSlot<'static>>(state.objects.entries.len());
    count.string(&state.sql);
    // Filtered capture may reserve a geometric Vec capacity and owns one
    // temporary active-kind spelling until preference construction completes.
    if let Some(active_id) = state.objects.active
        && let Some(entry) = state
            .objects
            .entries
            .iter()
            .find(|entry| entry.tab.id == active_id)
    {
        match &entry.tab.target.kind {
            ObjectKind::Table { name } | ObjectKind::SavedQuery { name, .. } => count.string(name),
            ObjectKind::Sql => {}
            ObjectKind::DraftTable { .. } => return Some(false),
        }
    }
    if let Some(navigation) = &state.objects.navigation {
        match &navigation.kind {
            ObjectKind::Table { name } | ObjectKind::SavedQuery { name, .. } => count.string(name),
            ObjectKind::Sql => {}
            ObjectKind::DraftTable { .. } => return Some(false),
        }
    }
    for entry in &state.objects.entries {
        // Session::empty performs only the existing small bounded presentation
        // allocations. Native row/schema/result contents never enter the copy.
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
            ObjectKind::SavedQuery { object_id, name } => {
                count.string(name);
                // A separate retained SavedQueryTarget owns its own spelling.
                count.string(name);
                let owner = if state.objects.active == Some(entry.tab.id) {
                    &state.saved_query
                } else {
                    let Some(Session::Saved { state: owner, .. }) = &entry.session else {
                        return Some(false);
                    };
                    owner.as_ref()
                };
                let resolved =
                    *object_id == request.target.object_id && *name == request.target.name;
                count.add(owner.sql_preference_copy_bytes()?);
                if resolved {
                    count.text(&request.proposed_sql);
                }
                // Raw draft/error buffers move at publication; they are already
                // charged in current State and are never copied during staging.
            }
            ObjectKind::Sql => {
                let design = if state.objects.active == Some(entry.tab.id) {
                    &state.query_design
                } else {
                    let Some(Session::Sql { design, .. }) = &entry.session else {
                        return Some(false);
                    };
                    design
                };
                count.add(design.rebind_preference_bytes()?);
            }
            ObjectKind::DraftTable { .. } => return Some(false),
        }
    }
    Some(count.finish()? <= MAX_RETAINED_BYTES)
}
