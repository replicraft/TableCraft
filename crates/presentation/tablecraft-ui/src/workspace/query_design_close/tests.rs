use super::*;
use crate::QueryDesignCompletion;
use crate::preview::{Preview, Scene};
use egui::{Event, Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::query::{build, import};

fn make_harness(saved: bool) -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(if saved {
        Scene::SavedQueryDesign
    } else {
        Scene::QuerySaveName
    });
    let mut state = std::mem::take(preview.state_mut());
    if !saved {
        assert!(state.cancel_query_creation_name());
        let sql = state.sql.as_str();
        let built = import(sql, &[]).unwrap();
        // NO_CLONE: The actual pending import must own its text independently
        // while the mutable editor remains available for stale-reply checks.
        let sql = sql.to_owned();
        let request = state.query_design_request().unwrap();
        assert!(state.begin_query_design_import(request, sql, 101));
        assert!(
            state
                .set_query_design_built(101, built, Vec::new())
                .is_some()
        );
    }
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 780.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

fn edit(harness: &mut Harness<'_, (State, Vec<Action>)>, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .focus();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Paste(text.into()));
    harness.run_steps(3);
}

#[test]
fn real_parsed_scratch_and_saved_design_edits_require_exact_save_discard_or_cancel() {
    for saved in [false, true] {
        let mut harness = make_harness(saved);
        assert!(!harness.state().0.has_modified_query_design());
        edit(&mut harness, "Answer: 101");
        let tab = harness.state().0.active_object().unwrap();
        assert!(harness.state().0.has_modified_query_design());
        assert!(!harness.state_mut().0.close_object(&tab));
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Button, "Cancel")
            .click();
        harness.run_steps(2);
        assert_eq!(harness.state().0.active_object().as_ref(), Some(&tab));
        assert!(harness.state().0.has_modified_query_design());
        assert!(harness.state().1.iter().all(|action| !matches!(
            action,
            Action::ReviewQueryCreation { .. } | Action::ReviewSavedQuerySql { .. }
        )));

        assert!(!harness.state_mut().0.close_object(&tab));
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Button, "Discard")
            .click();
        harness.run_steps(2);
        assert!(
            !harness
                .state()
                .0
                .object_tabs()
                .iter()
                .any(|open| open.id == tab.id)
        );
        assert!(!harness.state().0.has_modified_query_design());
    }
}

#[test]
fn saved_design_refuses_stored_run_even_before_build_changes_the_sql_draft() {
    let mut harness = make_harness(true);
    edit(&mut harness, "Answer: 101");
    let state = &harness.state().0;
    assert!(!state.saved_query_sql_dirty());
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 99 AS [Answer];");
    assert!(!state.can_run_saved_query(state.selected_saved_target().unwrap(), &[]));
    assert!(state.query_design_run_action().is_some());
}

#[test]
fn name_cancel_after_real_build_revokes_close_intent_before_a_later_ordinary_save() {
    let mut harness = make_harness(false);
    edit(&mut harness, "Answer: 101");
    let state = &mut harness.state_mut().0;
    let tab = state.active_object().unwrap();
    assert!(state.request_query_design_close(&tab));
    let result = state
        .choose_query_design_close(QueryDesignCloseDecision::Save)
        .unwrap();
    let Action::BuildQueryDesign {
        request,
        draft,
        completion,
    } = state
        .begin_query_design_close_save(&result.request)
        .unwrap()
    else {
        panic!("actual Build required before Name");
    };
    let built = build(&draft, &[]).unwrap();
    assert!(state.begin_query_design_build(request, draft, completion, 102));
    assert_eq!(
        state.set_query_design_built(102, built, Vec::new()),
        Some(QueryDesignCompletion::Save)
    );
    assert!(state.query_creation_name_open());
    assert!(state.cancel_query_creation_name());
    let mut actions = Vec::new();
    state.finish_query_design_close_events(&mut actions);
    let [Action::QueryDesignClose { result: cancelled }] = actions.as_slice() else {
        panic!("one exact cancellation for deferred host action");
    };
    assert_eq!(cancelled.decision, QueryDesignCloseDecision::Cancel);
    assert_eq!(cancelled.request.as_ref(), result.request.as_ref());
    assert!(state.query_design_close.intent.is_none());
    assert!(state.begin_query_creation_name());
    assert!(state.set_query_creation_name("Later ordinary save".into()));
    let Action::ReviewQueryCreation { request } = state.submit_query_creation_name().unwrap()
    else {
        panic!("ordinary named request");
    };
    assert_eq!(request.sql.as_str(), "SELECT 101 AS [Answer];");
    assert!(state.query_design_close.intent.is_none());
    assert_eq!(state.active_object().as_ref(), Some(&tab));
}

#[test]
fn a_stale_close_save_cannot_build_or_arm_a_future_save() {
    let mut harness = make_harness(false);
    edit(&mut harness, "Answer: 101");
    let state = &mut harness.state_mut().0;
    let tab = state.active_object().unwrap();
    assert!(state.request_query_design_close(&tab));
    let result = state
        .choose_query_design_close(QueryDesignCloseDecision::Save)
        .unwrap();
    assert!(state.replace_query_sql("SELECT 103 AS [Newer]".into()));
    assert!(
        state
            .begin_query_design_close_save(&result.request)
            .is_none()
    );
    assert!(state.query_design_close.intent.is_none());
    assert_eq!(state.sql, "SELECT 103 AS [Newer]");
    assert_eq!(state.active_object().as_ref(), Some(&tab));
}
