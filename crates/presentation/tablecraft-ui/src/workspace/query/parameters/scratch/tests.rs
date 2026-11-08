//! Scratch acquisition retains the exact editor witness and previous results.
use super::*;
use crate::workspace::query::{
    provenance::ResultOrigin,
    tests::{harness, result, state},
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

const INSPECTION: u64 = 41;
const SQL: &str = "PARAMETERS [First] Text, [Second] Value; SELECT [First], [Second]";

type QueryHarness = Harness<'static, (State, Vec<Action>)>;

fn spec(name: &str, kind: &str) -> ParameterSpec {
    ParameterSpec {
        name: name.into(),
        kind: kind.into(),
        label: kind.into(),
        nullable: true,
    }
}

fn ready(sql: &str) -> State {
    let mut state = state();
    assert!(state.replace_query_sql(sql.into()));
    assert!(state.set_query_data(Some(sql.into()), result("previous successful result")));
    state
}

fn inspect(state: &mut State, sql: &str) {
    let captured = state.capture_query_draft().unwrap();
    assert_eq!(captured, sql);
    assert!(state.begin_query_parameter_inspection(INSPECTION, captured));
    assert!(state.query_results_previous());
}

fn open(sql: &str, declarations: Vec<ParameterSpec>) -> QueryHarness {
    let mut state = ready(sql);
    inspect(&mut state, sql);
    assert!(
        state
            .set_query_parameter_details(INSPECTION, sql, declarations)
            .is_none()
    );
    let mut harness = harness(state);
    harness.run_steps(2);
    harness
}

fn assert_previous(harness: &QueryHarness, rows: *const Vec<crate::Cell>, sql: &str) {
    let state = &harness.state().0;
    assert_eq!(state.query_data.as_ref().unwrap().rows.as_ptr(), rows);
    assert!(matches!(&state.query_result, ResultOrigin::Sql(original) if original == sql));
}

#[test]
fn ordered_widget_prompts_emit_one_typed_action_and_preserve_previous_result_ownership() {
    let mut harness = open(SQL, vec![spec("First", "text"), spec("Second", "value")]);
    let rows = harness.state().0.query_data.as_ref().unwrap().rows.as_ptr();
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Second")
            .is_none()
    );
    harness
        .get_by_role_and_label(Role::TextInput, "First")
        .focus();
    harness.run_steps(1);
    harness.event(egui::Event::Text("  Résumé 日本語 😀  ".into()));
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert!(harness.state().1.is_empty());
    assert_previous(&harness, rows, SQL);
    harness
        .get_by_role_and_label(Role::ComboBox, "Second")
        .click_accesskit();
    harness.run_steps(1);
    harness.get_by_label("Text").click_accesskit();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::CheckBox, "NULL")
        .click_accesskit();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(2);
    let [
        Action::RunParameterizedQuery {
            inspection_id,
            parameters,
        },
    ] = harness.state().1.as_slice()
    else {
        panic!("only the final confirmation may dispatch scratch execution");
    };
    assert_eq!(*inspection_id, INSPECTION);
    assert_eq!(parameters.len(), 2);
    assert_eq!(parameters[0].name, "First");
    assert_eq!(parameters[0].text, "  Résumé 日本語 😀  ");
    assert!(!parameters[0].is_null);
    assert_eq!(parameters[1].name, "Second");
    assert_eq!(parameters[1].kind, "text");
    assert!(parameters[1].is_null);
    assert_previous(&harness, rows, SQL);
    harness.run_steps(2);
    assert_eq!(harness.state().1.len(), 1);
}

#[test]
fn escape_closes_type_popup_before_cancelling_and_enter_on_cancel_never_runs() {
    let sql = "PARAMETERS [Value] Value; SELECT [Value]";
    let mut harness = open(sql, vec![spec("Value", "value")]);
    let rows = harness.state().0.query_data.as_ref().unwrap().rows.as_ptr();
    harness
        .get_by_role_and_label(Role::ComboBox, "Value")
        .click_accesskit();
    harness.run_steps(1);
    harness.key_press(Key::Escape);
    harness.run_steps(1);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Cancel")
            .is_some()
    );
    assert!(harness.state().1.is_empty());
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Cancel")
            .is_none()
    );
    assert!(harness.state().1.is_empty());
    assert_previous(&harness, rows, sql);
    assert!(
        !harness
            .state()
            .0
            .query_parameter_inspection_current(INSPECTION, sql)
    );
    assert!(!harness.state().0.query_results_previous());
}

#[test]
fn inspection_reply_rejects_sql_aba_document_replacement_and_exhausted_revision() {
    for change in 0..3 {
        let mut state = ready(SQL);
        let rows = state.query_data.as_ref().unwrap().rows.as_ptr();
        inspect(&mut state, SQL);
        match change {
            0 => {
                assert!(state.replace_query_sql("SELECT 2".into()));
                assert!(state.replace_query_sql(SQL.into()));
            }
            1 => state.database.as_mut().unwrap().document_revision += 1,
            2 => state.query_sql_revision = u64::MAX,
            _ => unreachable!(),
        }
        assert!(!state.query_parameter_inspection_current(INSPECTION, SQL));
        assert!(
            state
                .set_query_parameter_details(INSPECTION, SQL, vec![spec("First", "text")])
                .is_none()
        );
        let mut harness = harness(state);
        harness.run_steps(2);
        assert!(
            harness
                .query_by_role_and_label(Role::Button, "OK")
                .is_none()
        );
        assert_previous(&harness, rows, SQL);
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn oversized_literal_is_rejected_whole_and_cancel_preserves_prior_results() {
    let sql = "PARAMETERS [Name] Text; SELECT [Name]";
    let mut harness = open(sql, vec![spec("Name", "text")]);
    let rows = harness.state().0.query_data.as_ref().unwrap().rows.as_ptr();
    harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .focus();
    harness.run_steps(1);
    harness.event(egui::Event::Text("accepted".into()));
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Paste(
        "é".repeat(super::super::MAX_INPUT_BYTES),
    ));
    harness.run_steps(1);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Name")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("accepted")
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "OK")
            .accesskit_node()
            .is_disabled()
    );
    assert!(harness.state().1.is_empty());
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert_previous(&harness, rows, sql);
    assert!(
        !harness
            .state()
            .0
            .query_parameter_inspection_current(INSPECTION, sql)
    );
}

#[test]
fn malformed_declarations_and_wrong_reply_identity_cannot_open_or_replace_a_prompt() {
    let mut staged = ready(SQL);
    inspect(&mut staged, SQL);
    assert!(
        staged
            .set_query_parameter_details(INSPECTION, SQL, vec![spec("First", "text")])
            .is_none()
    );
    assert!(staged.scratch_parameter_prompt_open());
    assert!(
        staged
            .set_query_parameter_details(INSPECTION, SQL, vec![spec("Replacement", "text")])
            .is_none()
    );
    assert!(staged.scratch_parameter_prompt_open());
    let Phase::Prompt(body) = &staged.query_parameters.as_ref().unwrap().phase else {
        panic!("a repeated reply cannot dismiss accepted staging");
    };
    assert_eq!(body.declarations[0].name, "First");
    assert!(!staged.fail_query_parameter_inspection(INSPECTION));
    assert!(staged.scratch_parameter_prompt_open());

    let mut state = ready(SQL);
    inspect(&mut state, SQL);
    assert!(
        state
            .set_query_parameter_details(INSPECTION + 1, SQL, vec![spec("Wrong", "text")])
            .is_none()
    );
    assert!(state.query_parameter_inspection_current(INSPECTION, SQL));
    assert!(
        state
            .set_query_parameter_details(
                INSPECTION,
                SQL,
                vec![spec("First", "text"), spec("first", "text")],
            )
            .is_none()
    );
    assert!(!state.scratch_parameter_prompt_open());
    assert!(state.query_parameters.is_none());
    assert_eq!(
        state.error.as_deref(),
        Some(state.localizer.text("query-parameters-invalid").as_str())
    );

    for declarations in [
        vec![spec("First", "unrecognized-carrier")],
        (0..=super::super::MAX_PARAMETERS)
            .map(|index| spec(&format!("P{index}"), "text"))
            .collect(),
    ] {
        let mut state = ready(SQL);
        inspect(&mut state, SQL);
        assert!(
            state
                .set_query_parameter_details(INSPECTION, SQL, declarations)
                .is_none()
        );
        assert!(!state.scratch_parameter_prompt_open());
        assert!(state.query_parameters.is_none());
        assert!(state.query_data.is_some());
    }
}

#[test]
fn rejected_execution_begin_retains_confirmed_owner_and_exact_previous_result() {
    let sql = "SELECT 1";
    let mut state = ready(sql);
    let rows = state.query_data.as_ref().unwrap().rows.as_ptr();
    inspect(&mut state, sql);
    assert!(matches!(
        state.set_query_parameter_details(INSPECTION, sql, Vec::new()),
        Some(Action::RunParameterizedQuery { inspection_id: INSPECTION, parameters })
            if parameters.is_empty()
    ));
    assert!(!state.fail_query_parameter_inspection(INSPECTION));
    assert!(state.query_parameter_inspection_current(INSPECTION, sql));
    assert!(!state.begin_parameterized_query_run(INSPECTION, 0, sql.into(), Vec::new()));
    assert!(state.query_parameter_inspection_current(INSPECTION, sql));
    assert!(state.query_pending.is_none());
    state.error = None;

    let mut oversized = String::with_capacity(crate::workspace::objects::MAX_RETAINED_BYTES);
    oversized.push_str(sql);
    assert!(!state.begin_parameterized_query_run(INSPECTION, 42, oversized, Vec::new()));
    assert!(state.query_parameter_inspection_current(INSPECTION, sql));
    assert!(state.query_pending.is_none());
    assert_eq!(state.query_data.as_ref().unwrap().rows.as_ptr(), rows);
    assert!(matches!(&state.query_result, ResultOrigin::Sql(original) if original == sql));
}

#[test]
fn declaration_admission_accounts_for_existing_result_capacity_before_prompt_allocation() {
    let mut state = ready(SQL);
    inspect(&mut state, SQL);
    let original = state.query_data.as_ref().unwrap().rows.as_ptr();
    let target_bytes = crate::workspace::objects::MAX_RETAINED_BYTES - 64;
    let current = state.object_bytes().unwrap();
    let cell = &mut state.query_data.as_mut().unwrap().rows[0][0].text;
    let mut padding = String::with_capacity(cell.capacity() + target_bytes - current);
    padding.push_str(cell);
    *cell = padding;
    assert_eq!(state.object_bytes(), Some(target_bytes));
    assert!(
        state
            .set_query_parameter_details(INSPECTION, SQL, vec![spec("First", "text")])
            .is_none()
    );
    assert!(!state.scratch_parameter_prompt_open());
    assert!(state.query_parameters.is_none());
    assert_eq!(state.query_data.as_ref().unwrap().rows.as_ptr(), original);
    assert!(matches!(&state.query_result, ResultOrigin::Sql(original) if original == SQL));
    assert_eq!(
        state.error.as_deref(),
        Some(state.localizer.text("objects-memory-limit").as_str())
    );
}

#[test]
fn modal_blocks_background_run_and_focus_replacement_discards_only_unpublished_inputs() {
    let sql = "PARAMETERS [Name] Text; SELECT [Name]";
    let mut harness = open(sql, vec![spec("Name", "text")]);
    let rows = harness.state().0.query_data.as_ref().unwrap().rows.as_ptr();
    harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .focus();
    harness.run_steps(1);
    harness.event(egui::Event::Text("unpublished".into()));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
    assert!(harness.state().0.scratch_parameter_prompt_open());
    assert_previous(&harness, rows, sql);
    harness.state_mut().0.bump_focus_publication();
    harness.run_steps(2);
    assert!(!harness.state().0.scratch_parameter_prompt_open());
    assert!(harness.state().1.is_empty());
    assert_previous(&harness, rows, sql);
}

#[test]
fn actual_object_roundtrip_rejects_old_inspection_after_restoring_the_same_sql_and_results() {
    let mut state = ready(SQL);
    let tab = state.active_object().unwrap();
    let rows = state.query_data.as_ref().unwrap().rows.as_ptr();
    inspect(&mut state, SQL);
    assert!(
        state
            .prepare_object_open(crate::ObjectTarget {
                document_revision: 7,
                kind: crate::ObjectKind::SavedQuery {
                    object_id: 9,
                    name: "Typed prompts".into(),
                },
            })
            .is_some()
    );
    assert!(state.activate_object(&tab));
    assert_eq!(state.sql, SQL);
    assert_eq!(state.query_data.as_ref().unwrap().rows.as_ptr(), rows);
    assert!(!state.query_parameter_inspection_current(INSPECTION, SQL));
    assert!(
        state
            .set_query_parameter_details(INSPECTION, SQL, vec![spec("First", "text")])
            .is_none()
    );
    assert!(!state.scratch_parameter_prompt_open());
    assert_eq!(state.query_data.as_ref().unwrap().rows.as_ptr(), rows);
}

#[test]
fn update_handoff_requires_final_confirmation_and_moves_original_sql_and_inputs() {
    let sql =
        "PARAMETERS [First] Text, [Second] Long; UPDATE Items SET Name=[First] WHERE Id=[Second]";
    let mut harness = open(sql, vec![spec("First", "text"), spec("Second", "int32")]);
    let original_sql = harness
        .state()
        .0
        .query_parameters
        .as_ref()
        .unwrap()
        .sql
        .as_ptr();
    let rows = harness.state().0.query_data.as_ref().unwrap().rows.as_ptr();
    assert!(
        !harness
            .state_mut()
            .0
            .begin_query_update_review(INSPECTION, sql, 42, Vec::new())
    );
    harness
        .get_by_role_and_label(Role::TextInput, "First")
        .focus();
    harness.run_steps(1);
    harness.event(egui::Event::Text("'); DELETE FROM Items; --".into()));
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert!(harness.state().1.is_empty());

    let Phase::Prompt(body) = &harness.state().0.query_parameters.as_ref().unwrap().phase else {
        panic!("the second parameter must still need confirmation");
    };
    let unconfirmed = copy_inputs(&body.draft).unwrap();
    assert!(
        !harness
            .state_mut()
            .0
            .begin_query_update_review(INSPECTION, sql, 42, unconfirmed)
    );
    harness
        .get_by_role_and_label(Role::TextInput, "Second")
        .focus();
    harness.run_steps(1);
    harness.event(egui::Event::Text("7".into()));
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let [
        Action::RunParameterizedQuery {
            inspection_id: INSPECTION,
            parameters,
        },
    ] = harness.state().1.as_slice()
    else {
        panic!("only the final prompt confirmation dispatches inputs");
    };
    let confirmed = copy_inputs(parameters).unwrap();
    let Phase::Ready(body) = &harness.state().0.query_parameters.as_ref().unwrap().phase else {
        panic!("confirmed inputs must remain privately owned");
    };
    let original_inputs = body.draft.as_ptr();
    let original_text = body.draft[0].text.as_ptr();
    let before = harness.state().0.object_bytes().unwrap();
    let origin = harness
        .state_mut()
        .0
        .take_query_update_origin(INSPECTION, sql, &confirmed)
        .unwrap();
    assert_eq!(origin.sql.as_ptr(), original_sql);
    assert_eq!(origin.parameters.as_ptr(), original_inputs);
    assert_eq!(origin.parameters[0].text.as_ptr(), original_text);
    assert_eq!(origin.parameters, confirmed);
    assert_eq!(origin.parameters[0].text, "'); DELETE FROM Items; --");
    assert_eq!(origin.parameters[1].text, "7");
    assert!(harness.state().0.query_parameters.is_none());
    assert!(harness.state().0.object_bytes().unwrap() < before);
    assert_previous(&harness, rows, sql);
}

#[test]
fn update_handoff_rejects_changed_confirmed_inputs_and_stale_prompt_identity() {
    let sql = "PARAMETERS [Name] Text; UPDATE Items SET Name=[Name]";
    let mut harness = open(sql, vec![spec("Name", "text")]);
    let rows = harness.state().0.query_data.as_ref().unwrap().rows.as_ptr();
    harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .focus();
    harness.run_steps(1);
    harness.event(egui::Event::Text("confirmed".into()));
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let [Action::RunParameterizedQuery { parameters, .. }] = harness.state().1.as_slice() else {
        panic!("expected the confirmed parameter action");
    };
    let confirmed = copy_inputs(parameters).unwrap();
    for change in 0..5 {
        let mut altered = copy_inputs(&confirmed).unwrap();
        match change {
            0 => altered[0].name = "Other".into(),
            1 => altered[0].kind = "int32".into(),
            2 => altered[0].text.push('!'),
            3 => altered[0].is_null = true,
            4 => altered.clear(),
            _ => unreachable!(),
        }
        assert!(
            !harness
                .state_mut()
                .0
                .begin_query_update_review(INSPECTION, sql, 42, altered)
        );
        assert!(harness.state().0.query_parameters.is_some());
        assert!(!harness.state().0.query_update_dialog_open());
        assert_previous(&harness, rows, sql);
    }
    assert!(!harness.state_mut().0.begin_query_update_review(
        INSPECTION + 1,
        sql,
        42,
        copy_inputs(&confirmed).unwrap(),
    ));
    assert!(!harness.state_mut().0.begin_query_update_review(
        INSPECTION,
        sql,
        0,
        copy_inputs(&confirmed).unwrap(),
    ));
    harness.state_mut().0.bump_focus_publication();
    assert!(
        !harness
            .state_mut()
            .0
            .begin_query_update_review(INSPECTION, sql, 42, confirmed)
    );
    assert!(!harness.state().0.query_update_dialog_open());
    assert_previous(&harness, rows, sql);
}
