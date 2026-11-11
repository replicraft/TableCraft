//! Source-neutral SQL-origin guards; native AFTER verification remains host-owned.
use super::*;
use crate::{Database, SavedQuery, Table};

fn reviewed_schema() -> State {
    let mut state = reviewing_kind(QueryActionKind::Schema);
    assert!(state.set_query_action_review(
        REQUEST,
        QueryActionReview {
            id: REVIEW,
            document_revision: 7,
            table: "Items".into(),
            effect: QueryActionEffect::Schema(QuerySchemaEffect::CreateTable { column_count: 2 }),
        }
    ));
    state
}

fn fresh() -> Database {
    Database {
        document_revision: 8,
        title_path: None,
        title: "Keyboard query review".into(),
        format: "ACE 16".into(),
        tables: vec![Table {
            name: "Items".into(),
            local: true,
            system: false,
            storage: "Local".into(),
        }],
        queries: vec![SavedQuery {
            object_id: 9,
            name: "Typed prompts".into(),
            system: false,
        }],
    }
}

#[test]
fn failed_and_cancelled_schema_displays_acknowledge_without_issuing_after_publication() {
    for (outcome, saved) in [
        (EditSaveOutcome::Cancelled("cancelled".into()), false),
        (EditSaveOutcome::Failed("failed".into()), false),
        (EditSaveOutcome::Saved("known native AFTER".into()), true),
    ] {
        let mut state = reviewed_schema();
        assert!(state.begin_query_action_save(REVIEW, 71));
        assert!(state.set_query_action_save_outcome(71, outcome));
        assert!(state.can_acknowledge_query_action_save(71));
        assert!(!state.can_take_query_schema_publication(70));
        assert_eq!(state.can_take_query_schema_publication(71), saved);
        assert_eq!(state.take_query_schema_publication(71).is_some(), saved);
        assert_eq!(
            state.schema_reopen.as_ref().map(|read| read.revision),
            saved.then_some(7)
        );
        assert_eq!(state.uncertain_native_revision, None);
        if !saved {
            assert_eq!(state.query_schema_reopen_retry_revision(), None);
            assert!(state.acknowledge_query_action_save(71));
        }
    }
}

#[test]
fn guarded_schema_catalog_publication_preserves_sql_tab_and_refuses_wrong_delta_or_replay() {
    let mut state = reviewed_schema();
    let tab = state.active_object().unwrap();
    let sql = state.sql.clone();
    assert!(state.begin_query_action_save(REVIEW, 71));
    assert!(state.set_query_action_save_outcome(71, EditSaveOutcome::Saved("known AFTER".into())));
    let publication = state.take_query_schema_publication(71).unwrap();
    assert!(state.native_source_stale());
    assert!(!state.can_inspect_query_parameters());
    assert!(!state.begin_query_parameter_inspection(81, state.sql.clone()));
    let mut wrong = fresh();
    wrong.tables.push(Table {
        name: "Unexpected".into(),
        local: true,
        system: false,
        storage: "Local".into(),
    });
    assert!(!state.resolve_query_schema_after_reopen(&publication, wrong));
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
    assert_eq!(state.sql, sql);
    assert!(state.resolve_query_schema_after_reopen(&publication, fresh()));
    assert_eq!(state.database.as_ref().unwrap().document_revision, 8);
    assert_eq!(state.active_object().unwrap().id, tab.id);
    assert_eq!(state.sql, sql);
    assert!(state.query_data.is_none());
    assert!(!state.native_source_stale());
    assert!(state.can_inspect_query_parameters());
    assert!(!state.resolve_query_schema_after_reopen(&publication, fresh()));
}

#[test]
fn accepted_schema_terminal_survives_sql_edits_but_cannot_publish_over_the_changed_draft() {
    let mut state = reviewed_schema();
    assert!(state.begin_query_action_save(REVIEW, 71));
    assert!(state.replace_query_sql("SELECT 2".into()));
    assert!(state.set_query_action_save_outcome(71, EditSaveOutcome::Saved("known AFTER".into())));
    assert!(state.can_acknowledge_query_action_save(71));
    let publication = state.take_query_schema_publication(71).unwrap();
    assert!(!state.resolve_query_schema_after_reopen(&publication, fresh()));
    assert_eq!(state.sql, "SELECT 2");
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
    assert!(state.native_source_stale());
}

#[test]
fn known_after_retry_is_revision_bound_and_disabled_during_a_guarded_read() {
    let mut state = reviewed_schema();
    assert_eq!(state.query_schema_reopen_retry_revision(), None);
    assert!(state.begin_query_action_save(REVIEW, 71));
    assert!(state.set_query_action_save_outcome(71, EditSaveOutcome::Saved("known AFTER".into())));
    let _publication = state.take_query_schema_publication(71).unwrap();
    assert_eq!(state.query_schema_reopen_retry_revision(), Some(7));
    assert!(!state.begin_query_schema_reopen(8));
    assert!(state.begin_query_schema_reopen(7));
    assert_eq!(state.query_schema_reopen_retry_revision(), None);
    assert!(!state.begin_query_schema_reopen(7));
    state.fail_query_schema_reopen(8);
    assert_eq!(state.query_schema_reopen_retry_revision(), None);
    state.fail_query_schema_reopen(7);
    assert_eq!(state.query_schema_reopen_retry_revision(), Some(7));
    state.database.as_mut().unwrap().document_revision = 8;
    assert_eq!(state.query_schema_reopen_retry_revision(), None);
    assert!(!state.begin_query_schema_reopen(7));
}

#[test]
fn make_table_review_keeps_zero_rows_valid_and_publishes_only_after_acknowledged_save() {
    for inserted_rows in [0, 900] {
        let mut state = reviewing_sql(
            QueryActionKind::Schema,
            "SELECT Score INTO Items FROM SourceRows",
        );
        let effect = QueryActionEffect::Schema(QuerySchemaEffect::MakeTable {
            column_count: 1,
            inserted_rows,
        });
        assert_eq!(effect.matched_rows(), None);
        assert!(state.set_query_action_review(
            REQUEST,
            QueryActionReview {
                id: REVIEW,
                document_revision: 7,
                table: "Items".into(),
                effect,
            }
        ));
        assert!(state.begin_query_action_save(REVIEW, 71));
        assert!(!state.can_take_query_schema_publication(71));
        assert!(
            state.set_query_action_save_outcome(71, EditSaveOutcome::Saved("known AFTER".into()))
        );
        let publication = state.take_query_schema_publication(71).unwrap();
        assert!(state.resolve_query_schema_after_reopen(&publication, fresh()));
        assert!(!state.resolve_query_schema_after_reopen(&publication, fresh()));
    }
}
