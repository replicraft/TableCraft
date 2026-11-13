mod saved_error;
use super::*;
use crate::preview::{Preview, Scene};
use crate::{Cell, Data, ObjectTarget};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::query::DesignFieldMetadata;

fn state() -> State {
    let mut preview = Preview::new(Scene::QueryDesign);
    std::mem::take(preview.state_mut())
}

fn metadata() -> Vec<DesignTableMetadata> {
    vec![DesignTableMetadata {
        table: "Customers".into(),
        fields: ["CustomerID", "CustomerName"]
            .into_iter()
            .map(|name| DesignFieldMetadata {
                name: name.into(),
                scalar_type: None,
            })
            .collect(),
    }]
}

fn built(state: &State) -> BuiltQueryDesign {
    BuiltQueryDesign {
        draft: state.query_design_draft().unwrap().clone(),
        sql: "SELECT [CustomerID], [CustomerName] FROM [Customers]".into(),
    }
}

fn result() -> Data {
    Data {
        query_contents: None,
        source_revision: None,
        row_ordinals: None,
        table: "Query".into(),
        offset: 0,
        columns: vec!["CustomerID".into()],
        rows: vec![vec![Cell::scalar("42")]],
        has_more: false,
    }
}

#[test]
fn scratch_result_cannot_borrow_a_retained_designs_freshness_revision() {
    let mut state = state();
    let request = state.query_design_request().unwrap();
    let draft = state.query_design_draft().unwrap().clone();
    assert!(!draft.columns.is_empty());
    assert!(state.begin_query_design_build(
        request,
        Arc::new(draft),
        QueryDesignCompletion::Run,
        1
    ));
    assert_eq!(
        state.set_query_design_built(1, built(&state), metadata()),
        Some(QueryDesignCompletion::Run)
    );
    assert!(state.begin_query_design_run(2));
    assert!(state.set_query_design_results(2, result()));
    assert!(!state.query_results_previous());
    let design_sql = state.sql.clone();
    assert!(matches!(state.query_result, ResultOrigin::Design(_)));

    assert!(state.set_query_data(Some("SELECT 7".into()), result()));
    assert!(matches!(state.query_result, ResultOrigin::Sql(_)));
    assert_eq!(state.sql, design_sql);
    assert!(!state.query_design_results_previous());
    assert!(state.query_results_previous());
    let mut harness = Harness::builder().build_ui_state(
        |ui, state: &mut State| assert!(state.show(ui).is_empty()),
        state,
    );
    harness.run_steps(3);
    let previous = harness.state().localizer.text("query-results-previous");
    assert!(harness.query_by_label(&previous).is_some());
}

#[test]
fn build_failure_preserves_draft_and_run_requires_exact_instance_receipt() {
    let mut state = state();
    let original_sql = state.sql.clone();
    let original_draft = state.query_design_draft().unwrap().clone();
    let request = state.query_design_request().unwrap();
    assert!(state.begin_query_design_build(
        request.clone(),
        Arc::new(original_draft.clone()),
        QueryDesignCompletion::ShowSql,
        1
    ));
    state.fail_query_design(1, "Literal design validation failure".into());
    assert!(state.query_design_visible());
    assert_eq!(state.sql, original_sql);
    assert_eq!(state.query_design_draft(), Some(&original_draft));
    assert!(
        state
            .set_query_design_built(1, built(&state), metadata())
            .is_none()
    );

    assert!(state.begin_query_design_build(
        request,
        Arc::new(original_draft),
        QueryDesignCompletion::Run,
        2
    ));
    assert_eq!(
        state.set_query_design_built(2, built(&state), metadata()),
        Some(QueryDesignCompletion::Run)
    );
    assert!(state.query_design_results_visible());
    assert!(state.begin_query_design_run(3));
    assert!(state.cancel_query_design(3));
    assert!(!state.set_query_design_results(3, result()));
    assert!(state.query_data.is_none());
    let mut old_result = result();
    old_result.rows[0][0] = Cell::scalar("7");
    assert!(state.set_query_data(None, old_result));
    let rows = state.query_data.as_ref().unwrap().rows.as_ptr();
    assert!(state.begin_query_design_run(4));
    let mut malformed = result();
    malformed.offset = 1;
    assert!(!state.set_query_design_results(4, malformed));
    assert_eq!(state.query_data.as_ref().unwrap().rows.as_ptr(), rows);
    assert_eq!(state.query_data.as_ref().unwrap().rows[0][0].text, "7");
    assert!(state.query_design_results_previous());
    assert!(state.set_query_design_results(4, result()));
    assert_eq!(state.query_data.as_ref().unwrap().rows[0][0].text, "42");
    assert!(!state.query_design_results_previous());
    let context = egui::Context::default();
    state
        .query_selection
        .select(state.query_data.as_ref().unwrap(), 0, 0);
    assert!(state.can_copy_selection());
    assert!(state.can_zoom_cell());
    assert!(state.select_current_record(&context));
    assert!(state.record_selected());
    let rows = state.query_data.as_ref().unwrap().rows.as_ptr();
    assert!(state.show_query_design().is_none());
    assert!(state.query_design_visible());
    assert_eq!(state.query_data.as_ref().unwrap().rows.as_ptr(), rows);
    assert!(
        state.query_selection.row_selected(),
        "hidden results retain their selection"
    );
    assert!(state.selected_cell_text().is_none());
    assert!(!state.can_copy_selection());
    assert!(!state.copy_selected(&context));
    assert!(!state.can_zoom_cell());
    assert!(!state.can_select_record());
    assert!(!state.record_selected());

    let retained_draft = state.query_design_draft().unwrap().clone();
    let layout = state.capture_workspace_layout();
    let tab = state.active_object().unwrap();
    assert!(state.close_object(&tab));
    state
        .prepare_object_open(ObjectTarget {
            document_revision: tab.target.document_revision,
            kind: ObjectKind::Sql,
        })
        .unwrap();
    assert_ne!(state.active_object().unwrap().id, tab.id);
    assert!(!state.set_query_design_results(4, result()));
    assert!(state.query_data.is_none());

    state.database.as_mut().unwrap().document_revision += 1;
    assert!(state.restore_workspace_layout(layout, ObjectKind::Sql));
    assert_eq!(state.query_design_draft(), Some(&retained_draft));
    assert!(state.query_design_visible());
    let session = state.query_design.session.as_ref().unwrap();
    assert!(session.pending.is_none());
    assert!(session.metadata.is_empty());
    assert!(session.result_revision.is_none());
    assert!(state.query_data.is_none());
    assert!(!state.set_query_design_results(4, result()));

    state.database.as_mut().unwrap().tables.push(crate::Table {
        name: "External".into(),
        system: false,
        local: false,
        storage: "Local".into(),
    });
    state.show_query_design_tables();
    let request = state.query_design_request().unwrap();
    assert!(!state.can_describe_query_design(&request, "External"));
    // Kittest defaults to 250 ms per pointer event. A click has three events,
    // so use a normal frame interval to exercise an actual double-click.
    let mut harness = Harness::builder()
        .with_step_dt(1.0 / 60.0)
        .with_size(egui::vec2(1100.0, 780.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    assert!(
        harness.state().1.is_empty(),
        "opening selects without loading schema"
    );
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Add")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .query_all_by_role_and_label(Role::Button, "External")
            .all(|node| node.accesskit_node().is_disabled())
    );
    harness.get_by_role_and_label(Role::Button, "Add").click();
    harness.run_steps(1);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::DescribeQueryDesign { table, .. }] if table == "Customers")
    );

    harness.state_mut().1.clear();
    harness
        .get_all_by_role_and_label(Role::Button, "Customers")
        .find(|node| !node.accesskit_node().is_disabled())
        .unwrap()
        .click();
    harness.run_steps(1);
    assert!(harness.state().1.is_empty(), "single selection stays inert");
    harness
        .get_all_by_role_and_label(Role::Button, "Customers")
        .find(|node| !node.accesskit_node().is_disabled())
        .unwrap()
        .click();
    harness.run_steps(1);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::DescribeQueryDesign { table, .. }] if table == "Customers")
    );
    harness
        .get_all_by_role_and_label(Role::Button, "Customers")
        .find(|node| !node.accesskit_node().is_disabled())
        .unwrap()
        .click();
    harness.run_steps(1);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::DescribeQueryDesign { table, .. }] if table == "Customers"),
        "three row clicks activate only once"
    );
}

#[test]
fn actual_grid_edits_keep_later_or_rows_and_reject_oversized_paste_whole() {
    let mut state = state();
    state
        .query_design
        .session
        .as_mut()
        .unwrap()
        .draft
        .columns
        .clear();
    state.show_query_design_tables();
    let mut harness = Harness::builder()
        .with_step_dt(1.0 / 60.0)
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut State| {
                state.show(ui);
            },
            state,
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Close").click();
    harness.run_steps(1);
    assert!(
        harness
            .state()
            .query_design_draft()
            .unwrap()
            .columns
            .is_empty()
    );
    for expected in [0, 1, 1] {
        harness
            .get_by_role_and_label(Role::Button, "CustomerID")
            .click();
        harness.run_steps(1);
        assert_eq!(
            harness.state().query_design_draft().unwrap().columns.len(),
            expected
        );
    }
    assert_eq!(
        harness.state().query_design_draft().unwrap().columns[0].expression,
        tablecraft_builders::query::DesignExpression::Field(
            tablecraft_builders::query::DesignField {
                source_id: 1,
                column: "CustomerID".into(),
            }
        )
    );
    let id_field = harness
        .get_by_role_and_label(Role::Button, "CustomerID")
        .rect();
    let name_field = harness
        .get_by_role_and_label(Role::Button, "CustomerName")
        .rect();
    assert!(
        name_field.top() >= id_field.bottom(),
        "compact source fields form contiguous, nonoverlapping vertical rows"
    );
    assert!((id_field.left() - name_field.left()).abs() < 1.0);
    let expression = harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .rect();
    let criterion = harness
        .get_all_by_role_and_label(Role::TextInput, "Criterion")
        .next()
        .unwrap()
        .rect();
    assert!(expression.top() > name_field.bottom());
    assert!(
        criterion.bottom() < harness.ctx.content_rect().bottom(),
        "compact QBE criteria stay inside the viewport"
    );

    harness
        .state_mut()
        .query_design
        .session
        .as_mut()
        .unwrap()
        .draft
        .columns[0]
        .criteria = vec!["1".into(), "2".into()];
    harness.run();

    let input = harness
        .get_all_by_role_and_label(Role::TextInput, "Criterion")
        .next()
        .unwrap();
    input.focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_all_by_role_and_label(Role::TextInput, "Criterion")
        .next()
        .unwrap()
        .type_text("3");
    harness.run();
    assert_eq!(
        harness.state().query_design_draft().unwrap().columns[0].criteria,
        ["3", "2"]
    );

    harness
        .input_mut()
        .events
        .push(egui::Event::Paste("x".repeat(MAX_DESIGN_TEXT_BYTES + 1)));
    harness.run();
    assert_eq!(
        harness.state().query_design_draft().unwrap().columns[0].criteria,
        ["3", "2"]
    );
    assert_eq!(
        harness
            .state()
            .query_design
            .session
            .as_ref()
            .unwrap()
            .error
            .as_deref(),
        Some(harness.state().localizer.text("qbe-limit").as_str())
    );

    let session = harness.state_mut().query_design.session.as_mut().unwrap();
    let used = tablecraft_builders::query::draft_text_bytes(&session.draft).unwrap();
    let old_length = session.draft.columns[0].criteria[0].len();
    session.draft.columns[0].criteria[0] = "x".repeat(MAX_DESIGN_TEXT_BYTES - used + old_length);
    session.error = None;
    session.click_pair = None;
    let retained = session.draft.clone();
    let revision = session.revision;
    assert_eq!(
        tablecraft_builders::query::draft_text_bytes(&retained).unwrap(),
        MAX_DESIGN_TEXT_BYTES
    );
    harness.run();
    for _ in 0..2 {
        harness
            .get_by_role_and_label(Role::Button, "CustomerID")
            .click();
        harness.run_steps(1);
    }
    let session = harness.state().query_design.session.as_ref().unwrap();
    assert_eq!(
        session.draft, retained,
        "typed field addition retains the whole bounded draft"
    );
    assert_eq!(session.revision, revision);
    assert_eq!(
        session.error.as_deref(),
        Some(harness.state().localizer.text("qbe-limit").as_str())
    );
}

#[test]
fn design_sql_replacement_advances_scratch_revision_and_exhaustion_preserves_both_drafts() {
    for exhausted in [false, true] {
        let mut state = state();
        let original_sql = state.sql.clone();
        let original_draft = state.query_design_draft().unwrap().clone();
        let request = state.query_design_request().unwrap();
        assert!(state.begin_query_design_build(
            request,
            Arc::new(original_draft.clone()),
            QueryDesignCompletion::ShowSql,
            1,
        ));
        if exhausted {
            state.query_sql_revision = u64::MAX;
        }
        let revision = state.query_sql_revision();
        let mut proposed = built(&state);
        proposed.sql.push(';');
        let proposed_sql = proposed.sql.clone();
        let completed = state.set_query_design_built(1, proposed, metadata());
        if exhausted {
            assert_eq!(completed, None);
            assert_eq!(state.sql, original_sql);
            assert_eq!(state.query_design_draft(), Some(&original_draft));
            assert_eq!(state.query_sql_revision, u64::MAX);
        } else {
            assert_eq!(completed, Some(QueryDesignCompletion::ShowSql));
            assert_eq!(state.sql, proposed_sql);
            assert_eq!(state.query_sql_revision(), revision.map(|value| value + 1));
        }
    }
}

#[test]
fn shared_submitted_draft_keeps_complete_logical_retention_charge() {
    let mut state = state();
    let before = state.query_design.retained_bytes().unwrap();
    let Action::BuildQueryDesign {
        request,
        draft,
        completion,
    } = state.query_design_run_action().unwrap()
    else {
        panic!("the current Design must issue its bounded build action");
    };
    let baseline = state
        .query_design
        .session
        .as_ref()
        .unwrap()
        .baseline
        .snapshot()
        .unwrap();
    assert!(Arc::ptr_eq(baseline, &draft));
    let baseline_charge = retention::shared_draft_bytes(&draft).unwrap();
    assert!(before >= baseline_charge);

    // NO_CLONE: The observer retains the real submitted immutable handle across
    // cancellation; the State independently holds its baseline/pending owner.
    let submitted = Arc::clone(&draft);
    assert!(state.begin_query_design_build(request, draft, completion, 77));
    let pending = &state.query_design.session.as_ref().unwrap().pending;
    let PendingKind::Build { draft, .. } = &pending.as_ref().unwrap().kind else {
        panic!("the pending state must own the submitted draft");
    };
    assert!(Arc::ptr_eq(draft, &submitted));
    assert_eq!(
        state.query_design.retained_bytes(),
        Some(before),
        "a baseline allocation already charged in full is not charged twice"
    );
    assert!(state.cancel_query_design(77));
    assert_eq!(state.query_design.retained_bytes(), Some(before));
    assert_eq!(state.query_design_draft(), Some(submitted.as_ref()));

    // NO_CLONE: This control deliberately supplies a genuinely distinct equal
    // worker snapshot, proving all of its retained storage is additionally paid.
    let independent = Arc::new(submitted.as_ref().clone());
    assert!(!Arc::ptr_eq(&submitted, &independent));
    let mut payload = crate::workspace::objects::memory::Counter::new(
        size_of::<QueryDesignDraft>() + retention::SHARED_DRAFT_CONTROL_BYTES,
    );
    retention::count_draft(&mut payload, &independent);
    let expected_charge = payload.finish().unwrap();
    let request = state.query_design_request().unwrap();
    assert!(state.begin_query_design_build(request, independent, completion, 78));
    assert_eq!(
        state.query_design.retained_bytes(),
        before.checked_add(expected_charge),
        "distinct storage retains its complete vectors, text and sharing overhead"
    );
    assert!(state.cancel_query_design(78));
    assert_eq!(state.query_design.retained_bytes(), Some(before));
}
