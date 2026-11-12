//! Issued acquisition retains the built Design request through cancel and results.
use super::*;
use crate::preview::{Preview, Scene};
use crate::workspace::query::provenance::ResultOrigin;
use crate::workspace::{Action, Data};
use crate::{ParameterSpec, QueryDesignCompletion};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::sync::Arc;
use tablecraft_builders::query::build;

fn data(text: &str) -> Data {
    Data {
        query_contents: None,
        source_revision: None,
        row_ordinals: None,
        table: "Query results".into(),
        offset: 0,
        columns: vec!["Value".into()],
        rows: vec![vec![text.into()]],
        has_more: false,
    }
}

fn ready() -> State {
    let mut preview = Preview::new(Scene::QueryDesign);
    let mut state = std::mem::take(preview.state_mut());
    let request = state.query_design_request().unwrap();
    let session = state.query_design.session.as_ref().unwrap();
    // NO_CLONE: The submitted immutable build owns a separate draft while State
    // keeps the editable draft for its exact request/reply comparison.
    let draft = Arc::new(session.draft.clone());
    // NO_CLONE: The simulated service reply transfers owned descriptors back into
    // State; the existing session must retain its metadata until that reply lands.
    let metadata = session.metadata.clone();
    let built = build(&draft, &metadata).unwrap();
    assert!(state.begin_query_design_build(request, draft, QueryDesignCompletion::Run, 50));
    assert_eq!(
        state.set_query_design_built(50, built, metadata),
        Some(QueryDesignCompletion::Run),
    );
    assert!(state.set_query_data(None, data("previous")));
    state
}

fn inspect(state: &mut State) -> String {
    let sql = state.capture_query_draft().unwrap();
    // NO_CLONE: Acquisition consumes its SQL witness; the simulated reply retains
    // independent submitted text for exact stale-response verification.
    assert!(state.begin_query_parameter_inspection(51, sql.clone()));
    sql
}

#[test]
fn parameterless_design_uses_inspection_and_publishes_only_its_matching_design_result() {
    let mut state = ready();
    // NO_CLONE: Keep an independent expected draft across mutable acquisition,
    // cancellation and result publication to detect any lost user choices.
    let draft = state.query_design_draft().unwrap().clone();
    let sql = inspect(&mut state);
    assert!(matches!(
        state.set_query_parameter_details(51, &sql, Vec::new()),
        Some(Action::RunParameterizedQuery {
            inspection_id: 51,
            ..
        }),
    ));
    assert!(state.begin_parameterized_query_run(51, 52, sql, Vec::new()));
    assert!(state.query_pending.is_none());
    assert!(!state.set_query_run_data(53, data("stale")));
    assert_eq!(
        state.query_data.as_ref().unwrap().rows[0][0].text,
        "previous"
    );
    assert!(state.set_query_run_data(52, data("new complete result")));
    assert!(matches!(state.query_result, ResultOrigin::Design(_)));
    assert!(!state.query_design_results_previous());
    assert_eq!(state.query_design_draft(), Some(&draft));
    assert!(!state.set_query_run_data(52, data("duplicate")));
    assert_eq!(
        state.query_data.as_ref().unwrap().rows[0][0].text,
        "new complete result"
    );
}

#[test]
fn changed_draft_rejects_confirmed_old_acquisition_and_preserves_previous_result() {
    let mut state = ready();
    let sql = inspect(&mut state);
    assert!(
        state
            .set_query_parameter_details(51, &sql, Vec::new())
            .is_some()
    );
    state.query_design.session.as_mut().unwrap().revision += 1;
    assert!(!state.can_run_parameterized_query(51, &sql, &Vec::new()));
    assert!(!state.begin_parameterized_query_run(51, 52, sql, Vec::new()));
    assert!(state.query_design_run_request_id().is_none());
    assert_eq!(
        state.query_data.as_ref().unwrap().rows[0][0].text,
        "previous"
    );
}

#[test]
fn actual_prompt_cancel_keeps_design_draft_mode_and_complete_previous_results() {
    let mut state = ready();
    // NO_CLONE: Keep an independent expected draft across mutable acquisition,
    // cancellation and result publication to detect any lost user choices.
    let draft = state.query_design_draft().unwrap().clone();
    let sql = inspect(&mut state);
    assert!(
        state
            .set_query_parameter_details(
                51,
                &sql,
                vec![ParameterSpec {
                    name: "Minimum".into(),
                    kind: "int32".into(),
                    label: "Integer".into(),
                    nullable: true,
                }]
            )
            .is_none()
    );
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 900.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().0.parameter_prompt_open());
    assert!(harness.state().0.query_design_results_visible());
    assert_eq!(harness.state().0.query_design_draft(), Some(&draft));
    assert_eq!(
        harness.state().0.query_data.as_ref().unwrap().rows[0][0].text,
        "previous"
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn retained_parameter_name_allocation_can_refuse_acquisition_without_losing_the_draft() {
    let mut state = ready();
    let retained = state.object_bytes().unwrap();
    let maximum = crate::workspace::objects::MAX_RETAINED_BYTES;
    // Reserve spare capacity without changing SQL contents or allocating query rows.
    // Leave enough room for the small declaration envelope, but not its name text.
    let capacity = state.sql.capacity() + maximum - retained - 512;
    state.sql.reserve_exact(capacity - state.sql.len());
    assert!(state.object_bytes().unwrap() <= maximum);

    let sql = format!(
        "PARAMETERS [{}] LONG; SELECT [Customers].[CustomerID] FROM [Customers]",
        "P".repeat(1024),
    );
    let declared = tablecraft_builders::query::import_unbound(&sql).unwrap();
    state
        .query_design
        .session
        .as_mut()
        .unwrap()
        .draft
        .parameters = declared.parameters;
    // NO_CLONE: Keep an independent expected draft across mutable acquisition,
    // cancellation and result publication to detect any lost user choices.
    let draft = state.query_design_draft().unwrap().clone();
    assert!(state.object_bytes().unwrap() > maximum);
    assert!(state.capture_query_draft().is_none());
    assert!(state.query_parameters.is_none());
    assert_eq!(state.query_design_draft(), Some(&draft));
    assert_eq!(
        state.query_data.as_ref().unwrap().rows[0][0].text,
        "previous"
    );
    assert!(state.query_design_results_visible());
}
