//! Revalidate every old/new presentation owner before transferring any buffer.
use crate::workspace::{
    DocumentView, State,
    objects::{ObjectKind, ObjectTarget, Objects, session::Session},
    saved_query::{SavedQueryState, SavedQueryTarget},
};

fn target_matches(owner: &ObjectTarget, target: &SavedQueryTarget) -> bool {
    owner.document_revision == target.document_revision
        && matches!(&owner.kind, ObjectKind::SavedQuery { object_id, name }
            if *object_id == target.object_id && *name == target.name)
}

fn saved_matches(owner: &ObjectTarget, saved: &SavedQueryState) -> bool {
    saved
        .target
        .as_ref()
        .is_none_or(|target| target_matches(owner, target))
        && saved
            .sql
            .as_ref()
            .is_none_or(|draft| saved.target.is_some() && target_matches(owner, draft.target()))
}

fn session_matches(owner: &ObjectTarget, session: &Session) -> bool {
    match (&owner.kind, session) {
        (ObjectKind::Table { .. }, Session::Table { .. })
        | (ObjectKind::Sql, Session::Sql { .. }) => true,
        (ObjectKind::SavedQuery { .. }, Session::Saved { state, .. }) => {
            saved_matches(owner, state)
        }
        _ => false,
    }
}

fn active_matches(state: &State, owner: &ObjectTarget) -> bool {
    match &owner.kind {
        ObjectKind::Table { .. } => {
            matches!(state.view, DocumentView::Table | DocumentView::Design)
                && !state.saved_query.active
        }
        ObjectKind::SavedQuery { .. } => {
            state.view == DocumentView::Query
                && state.saved_query.active
                && saved_matches(owner, &state.saved_query)
        }
        ObjectKind::Sql => state.view == DocumentView::Query && !state.saved_query.active,
        ObjectKind::DraftTable { .. } => false,
    }
}

pub(super) fn current(state: &State) -> bool {
    let Some(active) = state.objects.active else {
        return false;
    };
    state.objects.entries.iter().all(|entry| {
        if entry.tab.id == active {
            entry.session.is_none() && active_matches(state, &entry.tab.target)
        } else {
            entry
                .session
                .as_ref()
                .is_some_and(|session| session_matches(&entry.tab.target, session))
        }
    })
}

pub(super) fn staged(state: &State, objects: &Objects, active_session: &Session) -> bool {
    if !current(state) {
        return false;
    }
    let Some(active) = objects.active else {
        return false;
    };
    objects.entries.iter().all(|entry| {
        if entry.tab.id == active {
            entry.session.is_none() && session_matches(&entry.tab.target, active_session)
        } else {
            entry
                .session
                .as_ref()
                .is_some_and(|session| session_matches(&entry.tab.target, session))
        }
    })
}
