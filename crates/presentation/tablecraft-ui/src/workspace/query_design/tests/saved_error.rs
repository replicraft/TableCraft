//! Failed native review is still visible when the SQL textbox is not the current view.
use crate::workspace::active_save::tests::{harness, inspected_state};
use crate::{Action, QueryDesignCompletion, SavedQuerySqlError};
use egui::{Event, Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::Queryable;
use tablecraft_builders::query::{build, import};

const REPORT: &str = "Alias changes are outside the native expression replacement profile.";

#[test]
fn a_real_authored_alias_review_failure_remains_visible_in_design_and_results_without_copying_it() {
    let (mut state, _) = inspected_state(true);
    let Action::ImportQueryDesign { request, sql } = state.show_query_design().unwrap() else {
        panic!("real saved owner import");
    };
    let built = import(&sql, &[]).unwrap();
    assert!(state.begin_query_design_import(request, sql, 101));
    assert!(
        state
            .set_query_design_built(101, built, Vec::new())
            .is_some()
    );
    let mut harness = harness(state);
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .focus();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Paste("RenamedAnswer: 42".into()));
    harness.run_steps(2);
    let state = &mut harness.state_mut().0;
    let Action::BuildQueryDesign {
        request,
        draft,
        completion,
    } = state.query_design_save_action_for(true).unwrap()
    else {
        panic!("review uses actual current authored Design");
    };
    let built = build(&draft, &[]).unwrap();
    assert!(state.begin_query_design_build(request, draft, completion, 102));
    assert_eq!(
        state.set_query_design_built(102, built, Vec::new()),
        Some(QueryDesignCompletion::SaveExisting)
    );
    let request = state.begin_saved_query_sql_review(103).unwrap();
    assert!(request.proposed_sql.contains("RenamedAnswer"));
    assert!(state.fail_saved_query_sql(103, &request, REPORT.into()));
    assert!(state.query_design_visible());
    assert!(state.saved_query_sql_dirty());
    let Some(SavedQuerySqlError::Host(report)) = state.saved_query_sql_error() else {
        panic!("typed host review failure retained");
    };
    let pointer = report.as_ptr();
    harness.run_steps(2);
    assert!(harness.get_by_label(REPORT).rect().width() > 0.0);
    assert!(harness.state().1.is_empty());

    let state = &mut harness.state_mut().0;
    let Action::BuildQueryDesign {
        request,
        draft,
        completion,
    } = state.query_design_run_action().unwrap()
    else {
        panic!("Run remains attached to authored Design");
    };
    let built = build(&draft, &[]).unwrap();
    assert!(state.begin_query_design_build(request, draft, completion, 104));
    assert_eq!(
        state.set_query_design_built(104, built, Vec::new()),
        Some(QueryDesignCompletion::Run)
    );
    assert!(state.query_design_results_visible());
    harness.run_steps(2);
    assert!(harness.get_by_label(REPORT).rect().width() > 0.0);
    let Some(SavedQuerySqlError::Host(report)) = harness.state().0.saved_query_sql_error() else {
        panic!("view change must keep the same typed failure owner");
    };
    assert_eq!(report.as_ptr(), pointer);
    assert!(harness.state().0.saved_query_sql_dirty());
    assert!(harness.state().1.is_empty());
}
