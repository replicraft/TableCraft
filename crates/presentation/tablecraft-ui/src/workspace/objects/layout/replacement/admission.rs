//! Admit actual owned carrier copies, without halving quota for obsolete rows.
use super::super::{Preference, catalog};
use crate::{
    Database,
    workspace::{
        State,
        objects::{
            MAX_OBJECTS, MAX_RETAINED_BYTES, ObjectKind, Objects,
            memory::Counter,
            session::{Entry, Session},
        },
    },
};

pub(in crate::workspace::objects::layout) fn fits(state: &State, fresh: &Database) -> Option<bool> {
    let mut count = Counter::new(state.object_bytes()?);
    count.add(catalog::retained_bytes(state.database.as_ref()?)?);
    count.add(catalog::retained_bytes(fresh)?);
    count.add(size_of::<Objects>());
    count.count::<Entry>(state.objects.entries.len());
    count.count::<Preference>(MAX_OBJECTS);
    count.count::<super::publication::SqlSlot<'static>>(state.objects.entries.len());
    count.string(&state.sql);
    if let Some(active) = state
        .objects
        .entries
        .iter()
        .find(|entry| Some(entry.tab.id) == state.objects.active)
    {
        match &active.tab.target.kind {
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
        let empty = Session::empty(&entry.tab.target.kind);
        count.add(empty.retained_bytes()?);
        let active = state.objects.active == Some(entry.tab.id);
        match &entry.tab.target.kind {
            ObjectKind::Table { name } => {
                count.string(name);
                let totals = if active {
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
                let owner = if active {
                    &state.saved_query
                } else {
                    let Some(Session::Saved { state: owner, .. }) = &entry.session else {
                        return Some(false);
                    };
                    owner.as_ref()
                };
                count.add(owner.sql_preference_copy_bytes()?);
            }
            ObjectKind::Sql => {
                let design = if active {
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
