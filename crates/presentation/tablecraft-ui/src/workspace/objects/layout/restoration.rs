//! Shared source-free preference construction, without installing any view.
use super::super::{
    MAX_OBJECTS, NEXT_OBJECT_ID, ObjectKind, ObjectTab, ObjectTarget, Objects, Session,
    session::Entry,
};
use super::Preference;
use crate::{
    SavedQueryTarget,
    workspace::{query::parameters::copy_text, query_design::QueryDesign, totals::Totals},
};
use std::sync::atomic::Ordering;

pub(super) enum PrepareError {
    Allocation,
    Identity,
}

pub(super) fn prepare(
    preferences: Vec<Preference>,
    revision: u64,
    retained: Option<&Objects>,
) -> Result<Objects, PrepareError> {
    prepare_with_identity(preferences, revision, retained, false)
}

pub(super) fn prepare_after_save(
    preferences: Vec<Preference>,
    revision: u64,
) -> Result<Objects, PrepareError> {
    prepare_with_identity(preferences, revision, None, true)
}

fn prepare_with_identity(
    preferences: Vec<Preference>,
    revision: u64,
    retained: Option<&Objects>,
    retain_ids: bool,
) -> Result<Objects, PrepareError> {
    if preferences.len() > MAX_OBJECTS
        || retained.is_some_and(|old| old.entries.len() != preferences.len())
    {
        return Err(PrepareError::Identity);
    }
    let mut restored = Objects {
        revision: Some(revision),
        ..Objects::default()
    };
    restored
        .entries
        .try_reserve_exact(preferences.len())
        .map_err(|_| PrepareError::Allocation)?;
    for (position, preference) in preferences.into_iter().enumerate() {
        let id = match retained {
            Some(old) => {
                let entry = &old.entries[position];
                if entry.tab.target.kind != preference.kind {
                    return Err(PrepareError::Identity);
                }
                entry.tab.id
            }
            None if retain_ids => {
                if preference.id == 0
                    || restored
                        .entries
                        .iter()
                        .any(|entry| entry.tab.id == preference.id)
                {
                    return Err(PrepareError::Identity);
                }
                preference.id
            }
            None => {
                NEXT_OBJECT_ID
                    .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                    .map_err(|_| PrepareError::Identity)?
                    + 1
            }
        };
        let target = ObjectTarget {
            document_revision: revision,
            kind: preference.kind,
        };
        let mut session = Session::empty(&target.kind);
        match &mut session {
            Session::DraftTable { .. } => return Err(PrepareError::Identity),
            Session::Table { design, totals, .. } => {
                *design = preference.design;
                *totals = Totals::restore(preference.totals);
            }
            Session::Saved { state, .. } => {
                let ObjectKind::SavedQuery { object_id, name } = &target.kind else {
                    return Err(PrepareError::Identity);
                };
                state.restore_preferences(
                    SavedQueryTarget {
                        document_revision: revision,
                        object_id: *object_id,
                        name: copy_text(name).ok_or(PrepareError::Allocation)?,
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
            tab: ObjectTab { id, target },
            session: Some(session),
            offset: preference.offset,
            retained_bytes,
        });
    }
    Ok(restored)
}
