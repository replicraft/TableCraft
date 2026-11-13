//! Freshness advances independently of exact authored/stored-model equality.
use super::*;
use crate::preview::{Preview, Scene};
use crate::workspace::active_save::tests::inspected_state;
use crate::{Action, State};
use egui::{Event, Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::query::{
    DesignFieldMetadata, DesignTableMetadata, build, field_cell_text, import, validate_metadata,
};

fn state() -> State {
    let mut preview = Preview::new(Scene::SavedQueryDesign);
    std::mem::take(preview.state_mut())
}

fn make_harness(state: State) -> Harness<'static, State> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 780.0))
        .build_ui_state(
            |ui, state: &mut State| {
                drop(state.show(ui));
            },
            state,
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

fn edit(harness: &mut Harness<'_, State>, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .focus();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Paste(text.into()));
    harness.run_steps(2);
}

fn complete_build(state: &mut State, action: Action, id: u64) {
    let Action::BuildQueryDesign {
        request,
        draft,
        completion,
    } = action
    else {
        panic!("actual source-bound Build required");
    };

    let built = build(&draft, &[]).unwrap();

    assert!(state.begin_query_design_build(request, draft, completion, id));
    assert_eq!(
        state.set_query_design_built(id, built, Vec::new()),
        Some(completion)
    );
}

#[test]
fn unchanged_run_and_build_advance_freshness_without_dirtying_or_blocking_close() {
    let mut state = state();
    let before = state.query_design_request().unwrap();
    assert!(!state.has_modified_query_design());
    let action = state.query_design_run_action().unwrap();
    complete_build(&mut state, action, 201);
    assert_ne!(
        state.query_design_request().unwrap().draft_revision,
        before.draft_revision
    );
    assert!(!state.has_modified_query_design());
    assert!(state.show_query_design().is_none());
    assert!(state.query_design_visible());
    let action = state.show_query_design_sql().unwrap();
    complete_build(&mut state, action, 202);
    assert!(!state.has_modified_query_design());
    assert!(!state.saved_query_sql_dirty());
    let tab = state.active_object().unwrap();
    assert!(state.close_object(&tab));
    assert!(!state.query_design_close_open());
}

#[test]
fn actual_field_escape_restores_clean_model_even_though_edit_revision_keeps_advancing() {
    let mut harness = make_harness(state());
    let original_revision = harness
        .state()
        .query_design_request()
        .unwrap()
        .draft_revision;

    edit(&mut harness, "OtherAnswer: 101");
    assert!(harness.state().has_modified_query_design());
    assert!(!harness.state().saved_query_sql_dirty());

    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert_eq!(
        field_cell_text(&harness.state().query_design_draft().unwrap().columns[0]).unwrap(),
        "Answer: 99"
    );
    assert_ne!(
        harness
            .state()
            .query_design_request()
            .unwrap()
            .draft_revision,
        original_revision
    );
    assert!(!harness.state().has_modified_query_design());
    let tab = harness.state().active_object().unwrap();
    assert!(harness.state_mut().close_object(&tab));
}

#[test]
fn changed_run_keeps_exact_baseline_and_shared_rebind_does_not_clean_the_source() {
    let mut harness = make_harness(state());

    edit(&mut harness, "Answer: 101");
    let action = harness.state().query_design_run_action().unwrap();
    complete_build(harness.state_mut(), action, 203);
    let state = harness.state();
    assert!(state.has_modified_query_design());
    let session = state.query_design.session.as_ref().unwrap();
    let baseline = session.baseline.snapshot().unwrap();
    let submitted = session.submitted.as_ref().unwrap();
    assert_ne!(baseline.as_ref(), submitted.draft.as_ref());
    let accepted_revision = submitted.text_revision;

    let preferences = state.query_design.preferences().unwrap();
    let mut destination = QueryDesign::from_preferences(preferences);
    let transferred = destination.session.as_ref().unwrap();
    assert!(Arc::ptr_eq(
        baseline,
        transferred.baseline.snapshot().unwrap()
    ));
    assert!(Arc::ptr_eq(
        &submitted.draft,
        &transferred.submitted.as_ref().unwrap().draft
    ));
    assert!(destination.is_dirty());

    // This unit checks the private post-publication comparison owner only.
    // Native save authority and full-image publication are exercised by the
    // separate real Desktop Save/Save As tests; no receipt is synthesized here.
    assert!(destination.resolve_saved_text(accepted_revision));
    assert!(!destination.is_dirty());
    assert!(state.query_design.is_dirty());
    assert_eq!(
        field_cell_text(&state.query_design_draft().unwrap().columns[0]).unwrap(),
        "Answer: 101"
    );
}

#[test]
fn importing_already_dirty_saved_sql_keeps_an_unknown_stored_model() {
    let (mut state, _) = inspected_state(true);
    assert!(state.set_saved_query_sql_text("SELECT 101 AS [Answer];".into()));
    let Action::ImportQueryDesign { request, sql } = state.show_query_design().unwrap() else {
        panic!("dirty authored SQL import");
    };
    let built = import(&sql, &[]).unwrap();
    assert!(state.begin_query_design_import(request, sql, 204));
    assert!(
        state
            .set_query_design_built(204, built, Vec::new())
            .is_some()
    );
    assert!(matches!(
        state.query_design.session.as_ref().unwrap().baseline,
        Baseline::Unknown
    ));
    assert!(state.query_design.is_dirty());
    assert!(state.saved_query_sql_dirty());
    assert_eq!(state.saved_query_sql().unwrap().0, "SELECT 42 AS [Answer];");
}

#[test]
fn hidden_design_from_an_older_sql_revision_cannot_certify_a_sql_origin_save() {
    let (mut state, _) = inspected_state(true);
    let Action::ImportQueryDesign { request, sql } = state.show_query_design().unwrap() else {
        panic!("saved source import");
    };
    let built = import(&sql, &[]).unwrap();
    assert!(state.begin_query_design_import(request, sql, 205));
    assert!(
        state
            .set_query_design_built(205, built, Vec::new())
            .is_some()
    );

    let mut harness = make_harness(state);

    edit(&mut harness, "Answer: 101");
    let action = harness.state_mut().show_query_design_sql().unwrap();
    complete_build(harness.state_mut(), action, 206);
    assert!(
        harness
            .state_mut()
            .set_saved_query_sql_text("SELECT 42 AS [Answer];".into())
    );
    let state = harness.state();
    assert!(!state.saved_query_sql_dirty());
    assert!(!state.has_modified_query_design());
    assert!(state.can_run_saved_query(state.selected_saved_target().unwrap(), &[]));

    let current_text_revision = state.saved_query_sql().unwrap().2;
    let mut destination = QueryDesign::from_preferences(state.query_design.preferences().unwrap());
    assert!(destination.resolve_saved_text(current_text_revision));
    assert!(destination.session.is_none());
    assert!(state.query_design.session.is_some());
}

#[test]
fn import_refuses_complete_retention_growth_before_changing_sql_draft_or_revision() {
    let mut state = state();
    let metadata = vec![DesignTableMetadata {
        table: "Candidate".into(),
        fields: (0..255)
            .map(|index| DesignFieldMetadata {
                name: format!("Field_{index:03}_with_a_long_but_valid_native_identifier"),
                scalar_type: None,
            })
            .collect(),
    }];
    validate_metadata(&metadata).unwrap();

    let snapshot = state
        .query_design
        .session
        .as_ref()
        .unwrap()
        .baseline
        .snapshot()
        .unwrap();
    let snapshot_bytes = retention::shared_draft_bytes(snapshot).unwrap();
    let initial = state.object_bytes().unwrap();
    let room = 4096;
    assert!(snapshot_bytes < room);
    state.error = Some("x".repeat(MAX_RETAINED_BYTES - initial - room));
    assert!(state.object_bytes().unwrap() + snapshot_bytes < MAX_RETAINED_BYTES);

    let request = state.query_design_request().unwrap();
    let sql_pointer = state.saved_query_sql().unwrap().1.as_ptr();
    // NO_CLONE: The real pending import retains its independent source text
    // while the original editor and its rollback witness remain unchanged.
    let sql = state.query_design_sql().unwrap().to_owned();
    let built = import(&sql, &[]).unwrap();
    assert!(state.begin_query_design_import(request, sql, 207));
    let revision = state.query_design_request().unwrap().draft_revision;

    assert!(state.set_query_design_built(207, built, metadata).is_none());
    assert_eq!(
        state.query_design_request().unwrap().draft_revision,
        revision
    );
    assert_eq!(state.saved_query_sql().unwrap().1.as_ptr(), sql_pointer);
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 99 AS [Answer];");
    assert_eq!(
        state.error.as_deref(),
        Some(state.localizer.text("objects-memory-limit").as_str())
    );
    assert_eq!(state.query_design_request_id(), Some(207));
}
