//! Synthetic presentation facts isolate stale-source admission; no native write claim.
use super::*;
use crate::workspace::number_column::tests::interaction::state;
use crate::{
    DeleteTarget, InsertTarget, NumberColumnCloseDecision, ObjectKind, ObjectTarget,
    QueryActionEffect, QueryActionKind, QueryActionReview, SavedQuery, SavedQuerySqlEligibility,
    SavedQueryTarget,
};

fn failed() -> (State, NumberColumnRequest, NumberColumnSaveIdentity) {
    let mut state = state(true);
    let tab = state.active_object().unwrap();
    // A UI-only row carrier makes old-row eligibility positive before the stale
    // guard. The actual native ADD fixture remains empty in host tests.
    state.data.as_mut().unwrap().rows = vec![vec!["1".into()]];
    assert!(state.begin_number_column_inspection(&tab, 80).is_ok());
    assert!(state.set_number_column_availability(&tab, 80, true));
    assert!(state.begin_number_column_name(&tab, "Score".into()));
    let request = state.number_column_review_request().unwrap();
    assert!(state.begin_number_column_review(&request, 81));
    assert!(state.accept_number_column_review(&request, 81, 90));
    let identity = state
        .prepare_number_column_save(&request, 81, 90)
        .unwrap()
        .bind(100);
    assert!(!state.can_return_to_number_column_draft(identity));
    assert!(state.set_number_column_save_outcome(
        identity,
        EditSaveOutcome::Saved("Known replacement".into())
    ));
    assert!(!state.can_return_to_number_column_draft(identity));
    assert!(state.begin_number_column_reopen(identity));
    assert!(!state.can_return_to_number_column_draft(identity));
    assert!(state.fail_number_column_reopen(identity, "Whole-image witness differs".into()));
    (state, request, identity)
}

fn returned_and_discarded() -> State {
    let (mut state, request, identity) = failed();
    assert!(state.return_number_column_to_draft(
        &request,
        identity,
        Some(SaveCleanupNotice::RetainedPrivateJournal)
    ));
    assert!(state.request_number_column_close(&request.tab));
    assert!(!state.can_save_number_column_close());
    assert!(
        state
            .choose_number_column_close(NumberColumnCloseDecision::Discard)
            .is_some()
    );
    assert!(!state.number_column_open());
    state
}

#[test]
fn return_moves_exact_name_outcome_and_cleanup_without_allocation_even_at_payload_ceiling() {
    let (mut state, request, identity) = failed();
    state.sql = String::with_capacity(crate::workspace::objects::MAX_RETAINED_BYTES);
    let bytes = state.object_bytes().unwrap();
    assert!(state.return_number_column_to_draft(
        &request,
        identity,
        Some(SaveCleanupNotice::RetainedPrivateJournal)
    ));
    assert_eq!(state.object_bytes(), Some(bytes));
    assert!(state.number_column_save_status().is_none());
    assert_eq!(state.number_column_name().unwrap().1, "Score");
    assert!(!state.can_edit_number_column_name(&request.tab));
    let warning = state.number_column_saved_warning().unwrap();
    assert_eq!(warning.identity, identity);
    assert!(
        matches!(warning.outcome, Some(EditSaveOutcome::Saved(message)) if message == "Known replacement")
    );
    assert_eq!(warning.reopen_error, Some("Whole-image witness differs"));
    assert_eq!(
        warning.cleanup,
        Some(SaveCleanupNotice::RetainedPrivateJournal)
    );
    let mut foreign = request.tab.clone();
    foreign.target.kind = ObjectKind::Table {
        name: "Foreign".into(),
    };
    assert!(
        !state
            .number_column
            .discard(&foreign, request.draft_revision)
    );
    assert!(
        !state
            .number_column
            .discard(&request.tab, request.draft_revision + 1)
    );
    assert_eq!(state.number_column_name().unwrap().1, "Score");
    assert!(
        state
            .number_column
            .discard(&request.tab, request.draft_revision)
    );
    assert!(!state.number_column_open());
    assert_eq!(
        state.number_column_saved_warning_name().unwrap(),
        (&request.tab, "Score")
    );
    assert!(state.number_column_source_stale());
    state.database.as_mut().unwrap().document_revision += 1;
    state.retire_unowned_number_warning(Some(8));
    assert!(state.number_column.warning.is_none());
}

#[test]
fn discarded_name_keeps_stale_row_rename_and_table_save_guards_but_allows_navigation() {
    let mut state = returned_and_discarded();
    let tab = state.active_object().unwrap();
    let edit = state.data.as_ref().unwrap().edit_target(0, 0).unwrap();
    let insert = InsertTarget {
        document_revision: 7,
        table: "Table1".into(),
    };
    let delete = DeleteTarget {
        document_revision: 7,
        table: "Table1".into(),
        parent_ordinal: 0,
    };
    let warning = state.number_column.warning.take();
    assert!(state.can_edit_cell(&edit));
    assert!(state.can_insert_row(&insert));
    assert!(state.can_delete_row(&delete));
    state.number_column.warning = warning;
    assert!(!state.can_edit_cell(&edit));
    assert!(!state.can_insert_row(&insert));
    assert!(!state.can_delete_row(&delete));
    assert!(state.can_close_object(&tab));
    assert!(state.close_object(&tab));
    state.table_rename_available = true;
    let warning = state.number_column.warning.take();
    assert!(state.can_rename_table(&tab.target));
    state.number_column.warning = warning;
    assert!(!state.can_rename_table(&tab.target));
    state.table_save_available = true;
    state.set_table_creation_available(7, true);
    state.set_table_design_available(7, true);
    let warning = state.number_column.warning.take();
    assert!(state.can_create_table());
    assert!(state.can_create_table_design());
    assert!(state.open_initial_table_draft());
    assert!(state.can_save_table_draft());
    state.number_column.warning = warning;
    assert!(!state.can_save_table_draft());
    assert!(!state.request_table_draft_save());
    assert!(state.number_column_saved_warning().is_some());
}

#[test]
fn saved_sql_and_action_mutations_refuse_stale_source_while_read_intents_remain_available() {
    let mut state = returned_and_discarded();
    state.database.as_mut().unwrap().queries.push(SavedQuery {
        object_id: 90,
        name: "Q".into(),
        system: false,
    });
    let target = SavedQueryTarget {
        document_revision: 7,
        object_id: 90,
        name: "Q".into(),
    };
    state.saved_query_sql_edit_available = true;
    state.begin_saved_query_inspection(target.clone(), 201);
    assert!(state.set_saved_query_details(201, target.clone(), Vec::new()));
    let inspection = state
        .begin_saved_query_sql_inspection(target.clone(), 202)
        .unwrap();
    assert!(state.set_saved_query_sql_inspection(
        202,
        &inspection,
        Some("SELECT 42 AS [Answer];".into()),
        SavedQuerySqlEligibility::QualifiedExpressionSelect
    ));
    let warning = state.number_column.warning.take();
    assert!(state.can_edit_saved_query_sql());
    state.number_column.warning = warning;
    assert!(!state.can_edit_saved_query_sql());
    assert!(!state.set_saved_query_sql_text("SELECT 43 AS [Answer];".into()));
    assert!(state.can_run_saved_query(&target, &[]));
    state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap();
    let sql = "UPDATE [Table1] SET [ID] = 2;";
    assert!(state.replace_query_sql(sql.into()));
    assert!(state.begin_query_parameter_inspection(300, sql.into()));
    assert!(
        state
            .set_query_parameter_details(300, sql, Vec::new())
            .is_some()
    );
    assert!(state.can_run_parameterized_query(300, sql, &Vec::new()));
    assert!(!state.begin_query_action_review(300, sql, 301, Vec::new(), QueryActionKind::Update));
    let warning = state.number_column.warning.take();
    assert!(state.begin_query_action_review(300, sql, 301, Vec::new(), QueryActionKind::Update));
    assert!(state.set_query_action_review(
        301,
        QueryActionReview {
            id: 302,
            document_revision: 7,
            table: "Table1".into(),
            effect: QueryActionEffect::Update {
                matched_rows: 1,
                changed_rows: 1
            },
        }
    ));
    assert!(state.can_apply_query_action(302));
    state.number_column.warning = warning;
    assert!(!state.can_apply_query_action(302));
}
