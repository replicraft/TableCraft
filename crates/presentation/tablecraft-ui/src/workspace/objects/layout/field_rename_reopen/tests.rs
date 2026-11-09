//! UI publication represents an already-verified host handoff, not a witness.
use crate::workspace::field_rename::test_support::accepted;
use crate::{Database, SavedQuery, Table};

fn fresh(old: &Database) -> Database {
    Database {
        title_path: old.title_path.clone(),
        document_revision: old.document_revision + 1,
        title: old.title.clone(),
        format: old.format.clone(),
        tables: old
            .tables
            .iter()
            .map(|table| Table {
                name: table.name.clone(),
                local: table.local,
                system: table.system,
                storage: table.storage.clone(),
            })
            .collect(),
        queries: old
            .queries
            .iter()
            .map(|query| SavedQuery {
                object_id: query.object_id,
                name: query.name.clone(),
                system: query.system,
            })
            .collect(),
    }
}

#[test]
fn wrong_catalog_is_atomic_and_accepted_publication_keeps_identity_until_host_clears() {
    let (mut state, request) = accepted(true);
    let identity = state
        .prepare_field_rename_save(&request, 81, 90)
        .unwrap()
        .bind(100);
    let old_tabs = state.object_tabs();
    let old_bytes = state.object_bytes();
    let mut wrong = fresh(state.database.as_ref().unwrap());
    wrong.tables[0].name = "Other".into();
    assert!(!state.resolve_field_rename_after_reopen(&request, identity, wrong));
    assert_eq!(state.object_tabs(), old_tabs);
    assert_eq!(state.object_bytes(), old_bytes);
    assert_eq!(state.field_rename_proposal().unwrap().1, "Total");

    let database = fresh(state.database.as_ref().unwrap());
    assert!(state.resolve_field_rename_after_reopen(&request, identity, database));
    assert_eq!(state.active_object().unwrap().id, request.target.tab.id);
    assert_eq!(state.active_object().unwrap().target.document_revision, 8);
    assert!(state.data.is_none());
    assert!(state.schema.is_none());
    assert!(state.matches_field_rename_save(&request, identity));
    assert_eq!(state.field_rename_proposal().unwrap().1, "Total");
}

#[test]
fn genuine_unchanged_identity_uses_same_publication_but_never_accepts_a_save_report() {
    let (mut state, request) = accepted(false);
    let identity = state
        .prepare_unchanged_field_rename(&request, 81, 90)
        .unwrap()
        .bind();
    assert!(
        !state
            .set_field_rename_save_outcome(identity, crate::EditSaveOutcome::Saved("fake".into()))
    );
    let database = fresh(state.database.as_ref().unwrap());
    assert!(state.resolve_field_rename_after_reopen(&request, identity, database));
    assert_eq!(state.field_rename_proposal().unwrap().1, "Score");
    assert!(state.field_rename_save_status().unwrap().1.is_none());
}
