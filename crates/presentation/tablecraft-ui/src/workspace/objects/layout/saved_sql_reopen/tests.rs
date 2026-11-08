use super::*;
use crate::workspace::{ObjectTab, saved_query::InputPreferences};
use crate::{
    Cell, Data, ParameterInput, ParameterSpec, SavedQuery, SavedQuerySqlEligibility,
    SavedQuerySqlError, SavedQueryTarget,
};

fn state() -> State {
    let mut state = crate::workspace::objects::tests::state();
    state.saved_query_sql_edit_available = true;
    state.database.as_mut().unwrap().queries.extend([
        SavedQuery {
            object_id: 10,
            name: "Other".into(),
            system: false,
        },
        SavedQuery {
            object_id: 11,
            name: "Parameters".into(),
            system: false,
        },
    ]);
    state
}

fn target(id: u128, name: &str) -> SavedQueryTarget {
    SavedQueryTarget {
        document_revision: 7,
        object_id: id,
        name: name.into(),
    }
}

fn open_sql(
    state: &mut State,
    target: SavedQueryTarget,
    original: &str,
    proposed: Option<String>,
) -> ObjectTab {
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(state.set_saved_query_details(1, target.clone(), vec![]));
    let inspection = state.begin_saved_query_sql_inspection(target, 2).unwrap();
    assert!(state.set_saved_query_sql_inspection(
        2,
        &inspection,
        Some(original.into()),
        SavedQuerySqlEligibility::QualifiedConstantSelect
    ));
    if let Some(proposed) = proposed {
        assert!(state.set_saved_query_sql_text(proposed));
    }
    state.active_object().unwrap()
}

fn review(state: &mut State) -> SavedQuerySqlRequest {
    let request = state.begin_saved_query_sql_review(3).unwrap();
    assert!(state.set_saved_query_sql_review(3, &request, 70));
    request
}

fn fresh(state: &State) -> Database {
    let old = state.database.as_ref().unwrap();
    Database {
        document_revision: 8,
        title_path: old.title_path.clone(),
        title: old.title.clone(),
        format: old.format.clone(),
        tables: old
            .tables
            .iter()
            .map(|table| crate::Table {
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

fn table_data() -> Data {
    Data {
        source_revision: Some(7),
        row_ordinals: None,
        table: "A".into(),
        offset: 0,
        columns: vec!["Value".into()],
        rows: vec![vec![Cell::scalar("old result")]],
        has_more: false,
    }
}

#[test]
fn all_dirty_sql_text_errors_tabs_and_scratch_survive_only_target_baseline_resolves() {
    let mut state = state();
    let table = state
        .prepare_object_open(crate::workspace::objects::tests::table("A"))
        .unwrap()
        .tab;
    assert!(state.set_table_data(table_data()));
    let mut user_text = String::with_capacity(4096);
    user_text.push_str(" select 51 AS [Answer];\n");
    let other = open_sql(
        &mut state,
        target(10, "Other"),
        "SELECT 50 AS Answer",
        Some(user_text),
    );
    let other_revision = state.saved_query_sql().unwrap().2;
    let other_buffers = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    let failed = state.begin_saved_query_sql_review(30).unwrap();
    assert!(state.fail_saved_query_sql(30, &failed, "original failure retained".into()));
    let scratch = state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert!(state.replace_query_sql(" select [Value] from [A];\n".into()));
    let owner = open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some(" select 43 AS [Answer];\n".into()),
    );
    let request = review(&mut state);
    let before = state.object_tabs();
    state.busy = true;
    assert!(state.can_rebind_saved_query_sql_layout(&request));
    let database = fresh(&state);
    assert!(state.resolve_saved_query_sql_review_after_reopen(3, &request, 70, database));
    assert!(state.busy);
    assert_eq!(state.active_object().unwrap().id, owner.id);
    assert_eq!(state.database.as_ref().unwrap().document_revision, 8);
    let after = state.object_tabs();
    for (old, new) in before.iter().zip(&after) {
        assert_eq!(new.id, old.id);
        assert_eq!(new.target.kind, old.target.kind);
        assert_eq!(new.target.document_revision, 8);
    }
    assert_eq!(after.len(), before.len());
    for entry in &state.objects.entries {
        if let Some(session) = &entry.session {
            assert_eq!(entry.retained_bytes, session.retained_bytes());
        }
    }
    assert_eq!(state.saved_query_sql().unwrap().0, request.proposed_sql);
    assert_eq!(state.saved_query_sql().unwrap().1, request.proposed_sql);
    assert!(!state.saved_query_sql_dirty());
    assert!(!state.saved_query_sql_pending());
    assert!(state.has_modified_saved_query_sql());
    state.busy = false;
    let other = after.iter().find(|tab| tab.id == other.id).unwrap();
    assert!(state.activate_object(other));
    let (original, proposed, revision, _) = state.saved_query_sql().unwrap();
    assert_eq!(original, "SELECT 50 AS Answer");
    assert_eq!(proposed, " select 51 AS [Answer];\n");
    assert_eq!(revision, other_revision);
    assert_eq!(
        state
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .rebind_buffer_identity(),
        other_buffers
    );
    assert!(state.saved_query_sql_dirty());
    assert_eq!(
        state.saved_query_sql_error(),
        Some(&SavedQuerySqlError::Host(
            "original failure retained".into()
        ))
    );
    assert!(!state.saved_query_sql_pending());
    let scratch = after.iter().find(|tab| tab.id == scratch.id).unwrap();
    assert!(state.activate_object(scratch));
    assert_eq!(state.sql, " select [Value] from [A];\n");
    let table = after.iter().find(|tab| tab.id == table.id).unwrap();
    assert!(state.activate_object(table));
    assert!(state.data.is_none());
    assert!(state.active_object_load().unwrap().needs_load);
}

#[test]
fn parked_saved_owner_can_resolve_while_clean_table_stays_active() {
    let mut state = state();
    let owner = open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some("SELECT 43 AS Answer".into()),
    );
    let request = review(&mut state);
    let table = state
        .prepare_object_open(crate::workspace::objects::tests::table("A"))
        .unwrap()
        .tab;
    assert!(state.set_table_data(table_data()));
    state.busy = true;
    let database = fresh(&state);
    assert!(state.resolve_saved_query_sql_review_after_reopen(3, &request, 70, database));
    assert_eq!(state.active_object().unwrap().id, table.id);
    assert!(state.data.is_none());
    state.busy = false;
    let owner = state
        .object_tabs()
        .into_iter()
        .find(|tab| tab.id == owner.id)
        .unwrap();
    assert!(state.activate_object(&owner));
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 43 AS Answer");
    assert!(!state.saved_query_sql_dirty());
}

#[test]
fn parameter_preferences_and_submitted_spelling_survive_fresh_metadata_reload() {
    let mut state = state();
    let parameter_target = target(11, "Parameters");
    state.begin_saved_query_inspection(parameter_target.clone(), 40);
    assert!(state.set_saved_query_details(
        40,
        parameter_target.clone(),
        vec![ParameterSpec {
            name: "Minimum".into(),
            kind: "int32".into(),
            label: "LONG".into(),
            nullable: true,
        }]
    ));
    let tab = state.active_object().unwrap();
    state.saved_query.restore_preferences(
        parameter_target,
        Some(InputPreferences {
            inputs: vec![ParameterInput {
                name: "Minimum".into(),
                kind: "int32".into(),
                text: "7".into(),
                is_null: false,
            }],
            declarations: vec![("Minimum".into(), "int32".into())],
        }),
    );
    open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some("select 43 AS [Answer];\n".into()),
    );
    let request = review(&mut state);
    let database = fresh(&state);
    assert!(state.resolve_saved_query_sql_review_after_reopen(3, &request, 70, database));
    let new_target = state.selected_saved_target().unwrap().clone();
    state.begin_saved_query_inspection(new_target.clone(), 41);
    assert!(state.set_saved_query_details(41, new_target, vec![]));
    assert_eq!(state.saved_query_sql().unwrap().1, request.proposed_sql);
    let tab = state
        .object_tabs()
        .into_iter()
        .find(|new| new.id == tab.id)
        .unwrap();
    assert!(state.activate_object(&tab));
    let preferences = state.saved_query.input_preferences().unwrap();
    assert_eq!(preferences.inputs[0].text, "7");
    assert_eq!(
        preferences.declarations,
        [("Minimum".into(), "int32".into())]
    );
}

#[test]
fn wrong_catalog_or_review_refuses_without_touching_the_current_publication() {
    let mut state = state();
    open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some("SELECT 43 AS Answer".into()),
    );
    let request = review(&mut state);
    let before = state.object_tabs();
    let before_buffers = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    let mut database = fresh(&state);
    database.queries[1].name.push('x');
    assert!(!state.resolve_saved_query_sql_review_after_reopen(3, &request, 70, database));
    let database = fresh(&state);
    assert!(!state.resolve_saved_query_sql_review_after_reopen(3, &request, 71, database));
    let mut database = fresh(&state);
    database.document_revision = 7;
    assert!(!state.resolve_saved_query_sql_review_after_reopen(3, &request, 70, database));
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
    assert_eq!(state.object_tabs(), before);
    assert_eq!(
        state
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .rebind_buffer_identity(),
        before_buffers
    );
    assert_eq!(state.saved_query_sql().unwrap().0, "SELECT 42 AS Answer");
    assert_eq!(state.saved_query_sql().unwrap().1, request.proposed_sql);
    assert!(state.matches_saved_query_sql_review(3, &request, 70));
}

#[test]
fn large_spare_capacity_moves_without_duplicating_the_raw_draft() {
    let mut state = state();
    let mut proposed = String::with_capacity(MAX_RETAINED_BYTES / 2 + 1);
    proposed.push_str("SELECT 43 AS Answer");
    open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some(proposed),
    );
    let request = review(&mut state);
    let before = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    assert!(state.can_rebind_saved_query_sql_layout(&request));
    let database = fresh(&state);
    assert!(state.resolve_saved_query_sql_review_after_reopen(3, &request, 70, database));
    let after = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    assert_eq!(
        (after.2, after.3, after.4, after.5),
        (before.2, before.3, before.4, before.5)
    );
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 43 AS Answer");
    assert!(!state.saved_query_sql_dirty());
}

#[test]
fn large_old_row_cache_does_not_impose_a_half_quota_on_small_transfer() {
    let mut state = state();
    state
        .prepare_object_open(crate::workspace::objects::tests::table("A"))
        .unwrap();
    let mut cached = table_data();
    let mut text = String::with_capacity(MAX_RETAINED_BYTES * 3 / 4);
    text.push_str("cached native row");
    cached.rows[0][0].text = text;
    assert!(state.set_table_data(cached));
    open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some("SELECT 43 AS Answer".into()),
    );
    let request = review(&mut state);
    let before = state.object_bytes().unwrap();
    assert!(before > MAX_RETAINED_BYTES / 2);
    assert!(state.can_rebind_saved_query_sql_layout(&request));
    let database = fresh(&state);
    assert!(state.resolve_saved_query_sql_review_after_reopen(3, &request, 70, database));
    assert!(state.object_bytes().unwrap() < before / 2);
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 43 AS Answer");
}

#[test]
fn injected_prepublication_slot_refusal_preserves_all_original_buffers() {
    let mut state = state();
    open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some("SELECT 43 AS Answer".into()),
    );
    let request = review(&mut state);
    let before = state.object_tabs();
    let before_buffers = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    let database = fresh(&state);
    let mut prepared = capture::prepare(&mut state, &request, &database).unwrap();
    // Inject a wrong destination variant at the final publication boundary.
    // Collection must refuse before any current owned buffer is taken.
    prepared.active = crate::workspace::objects::session::Session::empty(&ObjectKind::Sql);
    assert!(!publication::publish(&mut state, prepared, database));
    assert_eq!(state.object_tabs(), before);
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
    assert_eq!(
        state
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .rebind_buffer_identity(),
        before_buffers
    );
    assert!(state.matches_saved_query_sql_review(3, &request, 70));
}

#[test]
fn stale_review_cannot_move_a_newer_user_buffer() {
    let mut state = state();
    open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some("SELECT 43 AS Answer".into()),
    );
    let request = review(&mut state);
    assert!(state.set_saved_query_sql_text("SELECT 44 AS Answer".into()));
    let before = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    let database = fresh(&state);
    assert!(!state.resolve_saved_query_sql_review_after_reopen(3, &request, 70, database));
    assert_eq!(
        state
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .rebind_buffer_identity(),
        before
    );
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 44 AS Answer");
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
}

#[test]
fn other_dirty_review_authority_invalidates_without_changing_its_text_revision() {
    let mut state = state();
    let other = open_sql(
        &mut state,
        target(10, "Other"),
        "SELECT 50 AS Answer",
        Some("SELECT 51 AS Answer".into()),
    );
    let old_review = state.begin_saved_query_sql_review(30).unwrap();
    assert!(state.set_saved_query_sql_review(30, &old_review, 71));
    let buffers = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some("SELECT 43 AS Answer".into()),
    );
    let request = review(&mut state);
    let database = fresh(&state);
    assert!(state.resolve_saved_query_sql_review_after_reopen(3, &request, 70, database));
    let other = state
        .object_tabs()
        .into_iter()
        .find(|new| new.id == other.id)
        .unwrap();
    assert!(state.activate_object(&other));
    assert_eq!(state.saved_query_sql().unwrap().1, old_review.proposed_sql);
    assert_eq!(
        state.saved_query_sql().unwrap().2,
        old_review.draft_revision
    );
    assert_eq!(
        state
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .rebind_buffer_identity(),
        buffers
    );
    assert!(!state.saved_query_sql_pending());
    assert!(!state.matches_saved_query_sql_review(30, &old_review, 71));
    assert!(state.saved_query_sql_dirty());
}

#[test]
fn foreign_draft_in_another_owner_refuses_without_taking_either_buffer() {
    let mut state = state();
    let owner = open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some("SELECT 43 AS Answer".into()),
    );
    let request = review(&mut state);
    let other = open_sql(
        &mut state,
        target(10, "Other"),
        "SELECT 50 AS Answer",
        Some("SELECT 51 AS Answer".into()),
    );
    open_sql(
        &mut state,
        target(11, "Parameters"),
        "SELECT 60 AS Answer",
        Some("SELECT 61 AS Answer".into()),
    );
    let foreign = state.saved_query.sql.take().unwrap();
    assert!(state.activate_object(&other));
    // Inject an existing draft from another query, without changing its binding.
    state.saved_query.sql = Some(foreign);
    let foreign_buffers = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    assert!(state.activate_object(&owner));
    let owner_buffers = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    let before = state.object_tabs();
    assert!(!state.can_rebind_saved_query_sql_layout(&request));
    let database = fresh(&state);
    assert!(!state.resolve_saved_query_sql_review_after_reopen(3, &request, 70, database));
    assert_eq!(state.object_tabs(), before);
    assert_eq!(
        state
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .rebind_buffer_identity(),
        owner_buffers
    );
    assert!(state.activate_object(&other));
    assert_eq!(
        state
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .rebind_buffer_identity(),
        foreign_buffers
    );
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 61 AS Answer");
}

#[test]
fn wrong_non_saved_destination_slot_refuses_before_any_owned_take() {
    let mut state = state();
    let owner = open_sql(
        &mut state,
        target(9, "Q"),
        "SELECT 42 AS Answer",
        Some("SELECT 43 AS Answer".into()),
    );
    let request = review(&mut state);
    let table = state
        .prepare_object_open(crate::workspace::objects::tests::table("A"))
        .unwrap()
        .tab;
    assert!(state.activate_object(&owner));
    let before = state.object_tabs();
    let buffers = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    let database = fresh(&state);
    let mut prepared = capture::prepare(&mut state, &request, &database).unwrap();
    let entry = prepared
        .objects
        .entries
        .iter_mut()
        .find(|entry| entry.tab.id == table.id)
        .unwrap();
    entry.session = Some(crate::workspace::objects::session::Session::empty(
        &ObjectKind::Sql,
    ));
    assert!(!publication::publish(&mut state, prepared, database));
    assert_eq!(state.object_tabs(), before);
    assert_eq!(
        state
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .rebind_buffer_identity(),
        buffers
    );
    assert!(state.matches_saved_query_sql_review(3, &request, 70));
}
