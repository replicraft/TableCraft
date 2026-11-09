use super::*;
use crate::workspace::{ObjectKind, ObjectTarget, State};
use crate::{Database, SavedQuery, Table};

fn state() -> State {
    let mut state = State {
        database: Some(Database {
            document_revision: 7,
            title_path: None,
            title: "Options".into(),
            format: "Ace2016".into(),
            tables: vec![Table {
                name: "T".into(),
                local: true,
                system: false,
                storage: "Local".into(),
            }],
            queries: vec![SavedQuery {
                object_id: 9,
                name: "Q".into(),
                system: false,
            }],
        }),
        database_options_available: true,
        ..State::default()
    };
    state.dismiss_backstage();
    state.synchronize_objects();
    state
}

fn observed(state: &mut State, value: NameAutoCorrectState, available: bool) {
    assert!(state.begin_database_options_inspection(7, 1));
    assert!(state.set_database_options_inspection(7, 1, value, available));
}

fn table() -> ObjectTarget {
    ObjectTarget {
        document_revision: 7,
        kind: ObjectKind::Table { name: "T".into() },
    }
}

fn accepted(state: &mut State) -> DatabaseOptionsSaveIdentity {
    observed(state, NameAutoCorrectState::Missing, true);
    assert!(state.request_database_options_close_confirmation());
    let request = state.approve_database_options_close().unwrap();
    assert!(state.database_options_close_request_current(request));
    assert!(state.begin_database_options_review(7, 2));
    state
        .prepare_database_options_save(7, 2, 3)
        .unwrap()
        .bind(4)
}

#[test]
fn stored_states_are_distinct_and_unobserved_is_never_off() {
    for value in [
        NameAutoCorrectState::Missing,
        NameAutoCorrectState::Absent,
        NameAutoCorrectState::ExplicitOff,
        NameAutoCorrectState::Other,
    ] {
        let mut state = state();
        assert_eq!(state.database_options_observation(), None);
        observed(&mut state, value, false);
        assert_eq!(state.database_options_observation(), Some(value));
        assert!(!state.can_disable_name_auto_correct());
    }
}

#[test]
fn cancelled_confirmation_does_not_close_or_authorize_and_nonce_never_repeats() {
    let mut state = state();
    let tab = state.prepare_object_open(table()).unwrap().tab;
    observed(&mut state, NameAutoCorrectState::Missing, true);
    assert!(state.request_database_options_close_confirmation());
    let stale = state
        .database_options
        .confirmation
        .as_ref()
        .unwrap()
        .request;
    assert!(state.cancel_database_options_close());
    assert_eq!(state.active_object(), Some(tab.clone()));
    assert!(!state.database_options_close_request_current(stale));
    assert!(!state.begin_database_options_review(7, 2));
    assert!(state.request_database_options_close_confirmation());
    let current = state.approve_database_options_close().unwrap();
    assert_ne!(stale.confirmation_id, current.confirmation_id);
    assert_eq!(state.active_object(), Some(tab));
}

#[test]
fn exact_graph_change_refuses_close_approval_without_mutating_tabs() {
    let mut state = state();
    let tab = state.prepare_object_open(table()).unwrap().tab;
    observed(&mut state, NameAutoCorrectState::Missing, true);
    assert!(state.request_database_options_close_confirmation());
    state.database_options.confirmation.as_mut().unwrap().tabs[0].id += 1;
    assert!(state.approve_database_options_close().is_none());
    assert_eq!(state.active_object(), Some(tab));
    assert!(state.cancel_database_options_close());
}

#[test]
fn prepared_start_refusal_preserves_exact_graph_and_permits_explicit_retry() {
    let mut state = state();
    let tab = state.prepare_object_open(table()).unwrap().tab;
    observed(&mut state, NameAutoCorrectState::Missing, true);
    assert!(state.request_database_options_close_confirmation());
    let request = state.approve_database_options_close().unwrap();
    assert!(state.begin_database_options_review(7, 2));
    drop(state.prepare_database_options_save(7, 2, 3).unwrap());
    assert_eq!(state.active_object(), Some(tab));
    assert!(state.database_options_close_request_current(request));
    assert!(state.can_disable_name_auto_correct());
}

#[test]
fn failed_full_witness_retains_graph_warning_and_blocks_mutations_until_fresh_revision() {
    let mut state = state();
    let tab = state.prepare_object_open(table()).unwrap().tab;
    let identity = accepted(&mut state);
    assert!(
        state.set_database_options_save_outcome(identity, EditSaveOutcome::Saved("saved".into()))
    );
    assert!(state.begin_database_options_reopen(identity));
    assert!(state.fail_database_options_reopen(identity, "changed bytes".into()));
    assert!(state.return_from_database_options_reopen(identity, None));
    assert_eq!(state.active_object(), Some(tab));
    assert!(state.native_mutation_blocked());
    assert!(!state.database_options_pending());
    assert!(state.database_options_save_status().is_some());
    state.retire_database_options_warning(Some(7));
    assert!(state.database_options_source_stale());
    state.retire_database_options_warning(Some(8));
    assert!(!state.database_options_source_stale());
}

#[test]
fn verified_options_publication_closes_persisted_tabs_but_preserves_scratch_tool() {
    let mut state = state();
    let persisted = state.prepare_object_open(table()).unwrap().tab;
    let scratch = state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    state.sql = "SELECT  literal spelling".into();
    let identity = accepted(&mut state);
    assert!(
        state.set_database_options_save_outcome(identity, EditSaveOutcome::Saved("saved".into()))
    );
    assert!(state.begin_database_options_reopen(identity));
    let mut fresh = self::state().database.take().unwrap();
    fresh.document_revision = 8;
    assert!(state.resolve_database_options_after_reopen(identity, fresh));
    let tabs = state.object_tabs();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].id, scratch.id);
    assert_eq!(tabs[0].target.document_revision, 8);
    assert_ne!(tabs[0].id, persisted.id);
    assert_eq!(state.sql, "SELECT  literal spelling");
    assert!(state.matches_database_options_save(identity));
    assert!(state.clear_database_options_save(identity));
}

#[test]
fn verified_options_publication_accepts_empty_graph_without_inventing_sql_tab() {
    let mut state = state();
    assert!(state.object_tabs().is_empty());
    let identity = accepted(&mut state);
    assert!(
        state.set_database_options_save_outcome(identity, EditSaveOutcome::Saved("saved".into()))
    );
    assert!(state.begin_database_options_reopen(identity));
    let mut fresh = self::state().database.take().unwrap();
    fresh.document_revision = 8;
    assert!(state.resolve_database_options_after_reopen(identity, fresh));
    assert!(state.object_tabs().is_empty());
    assert!(state.active_object().is_none());
}

#[test]
fn dirty_stored_query_blocks_confirmation_without_losing_typed_text() {
    let mut state = state();
    state.saved_query_sql_edit_available = true;
    let target = crate::SavedQueryTarget {
        document_revision: 7,
        object_id: 9,
        name: "Q".into(),
    };
    state.begin_saved_query_inspection(target.clone(), 10);
    assert!(state.set_saved_query_details(10, target.clone(), vec![]));
    let inspection = state.begin_saved_query_sql_inspection(target, 11).unwrap();
    assert!(state.set_saved_query_sql_inspection(
        11,
        &inspection,
        Some("SELECT 42 AS Answer;".into()),
        crate::SavedQuerySqlEligibility::QualifiedExpressionSelect,
    ));
    assert!(state.set_saved_query_sql_text(" SELECT 43 AS Answer; -- keep me".into()));
    let pointer = state.saved_query_sql().unwrap().1.as_ptr();
    observed(&mut state, NameAutoCorrectState::Missing, true);
    assert!(!state.request_database_options_close_confirmation());
    assert!(state.has_modified_saved_query_sql());
    assert_eq!(state.saved_query_sql().unwrap().1.as_ptr(), pointer);
}

#[test]
fn dirty_table_draft_blocks_confirmation_without_discarding_fields() {
    let mut state = state();
    observed(&mut state, NameAutoCorrectState::Missing, true);
    let mut editor = crate::workspace::table_draft::Editor::initial("Unrelated".into());
    editor
        .draft
        .add_named_field(tablecraft_builders::table::DraftFieldType::Number, "KeepMe")
        .unwrap();
    let pointer = editor.draft.fields()[1].name().as_ptr();
    state.table_draft = Some(editor);
    assert!(!state.request_database_options_close_confirmation());
    let editor = state.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields()[1].name(), "KeepMe");
    assert_eq!(editor.draft.fields()[1].name().as_ptr(), pointer);
}

#[test]
fn unsupported_save_host_still_observes_explicit_native_state() {
    let mut state = state();
    state.database_options_available = false;
    observed(&mut state, NameAutoCorrectState::Other, false);
    assert_eq!(
        state.database_options_observation(),
        Some(NameAutoCorrectState::Other)
    );
    assert!(!state.can_disable_name_auto_correct());
}

#[test]
fn wrong_catalog_reopen_refuses_without_dropping_current_tabs_or_sql() {
    let mut state = state();
    let tab = state.prepare_object_open(table()).unwrap().tab;
    state.sql = "  SELECT unchanged tool text".into();
    let pointer = state.sql.as_ptr();
    let identity = accepted(&mut state);
    assert!(
        state.set_database_options_save_outcome(identity, EditSaveOutcome::Saved("saved".into()))
    );
    assert!(state.begin_database_options_reopen(identity));
    let mut fresh = self::state().database.take().unwrap();
    fresh.document_revision = 8;
    fresh.tables[0].name = "Wrong".into();
    assert!(!state.resolve_database_options_after_reopen(identity, fresh));
    assert_eq!(state.active_object(), Some(tab));
    assert_eq!(state.sql.as_ptr(), pointer);
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
}

#[test]
fn returned_warning_paints_report_once_without_claiming_the_old_observation_is_current() {
    let mut state = state();
    let identity = accepted(&mut state);
    assert!(state.set_database_options_save_outcome(
        identity,
        EditSaveOutcome::Saved("accepted save report".into()),
    ));
    assert!(state.begin_database_options_reopen(identity));
    assert!(state.fail_database_options_reopen(identity, "fresh read failed".into()));
    assert!(state.return_from_database_options_reopen(
        identity,
        Some(SaveCleanupNotice::RetainedPrivateJournal)
    ));
    state.database_options_restore_settings();
    let old_observation = state.localizer.text("database-options-state-missing");
    let warning = state.localizer.text("database-options-saved-warning");
    let cleanup = state
        .localizer
        .text("editing-save-retained-private-journal");
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut State| {
                state.show(ui);
            },
            state,
        );
    crate::configure(&harness.ctx);
    harness.run();
    let painted: Vec<_> = harness
        .output()
        .shapes
        .iter()
        .filter_map(|shape| {
            let egui::Shape::Text(text) = &shape.shape else {
                return None;
            };
            Some(text.galley.job.text.as_str())
        })
        .collect();
    assert!(!painted.contains(&old_observation.as_str()));
    assert!(painted.contains(&warning.as_str()));
    assert!(painted.contains(&cleanup.as_str()));
    assert_eq!(
        painted
            .iter()
            .filter(|text| **text == "fresh read failed")
            .count(),
        1
    );
    assert_eq!(
        painted
            .iter()
            .filter(|text| **text == "accepted save report")
            .count(),
        1
    );
    // Suppression is presentation-only; exact source observation and retained
    // outcome are preserved, while new native mutations remain blocked.
    assert_eq!(
        harness.state().database_options_observation(),
        Some(NameAutoCorrectState::Missing)
    );
    assert!(harness.state().matches_database_options_save(identity));
    assert!(harness.state().native_mutation_blocked());
}
