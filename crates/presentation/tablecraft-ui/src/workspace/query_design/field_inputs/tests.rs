//! Authoring buffers revoke old execution rights without discarding user text.
use super::*;
use crate::preview::{Preview, Scene};
use crate::workspace::{ObjectKind, ObjectTarget, QueryDesignCompletion, State};
use egui::{Event, Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::sync::Arc;
use tablecraft_builders::query::field_cell_text;

fn state() -> State {
    let mut preview = Preview::new(Scene::QueryDesign);
    std::mem::take(preview.state_mut())
}

fn harness(state: State) -> Harness<'static, State> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut State| {
                state.show(ui);
            },
            state,
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

fn edit(harness: &mut Harness<'_, State>, index: usize, text: &str) {
    harness
        .get_all_by_role_and_label(Role::TextInput, "Field expression")
        .nth(index)
        .unwrap()
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Paste(text.into()));
    harness.run_steps(3);
}

fn raw(state: &State, index: usize) -> Option<&str> {
    state
        .query_design
        .session
        .as_ref()?
        .field_inputs
        .text(index)
}

#[test]
fn incomplete_alias_survives_blur_another_column_and_cancel_revokes_old_requests() {
    let mut harness = harness(state());
    let old_request = harness.state().query_design_request().unwrap();
    // NO_CLONE: This is the actual earlier submitted immutable snapshot; the
    // live editor must remain independently mutable to reject its stale action.
    let old_draft = Arc::new(harness.state().query_design_draft().unwrap().clone());
    let old_expression = field_cell_text(&old_draft.columns[0]).unwrap().into_owned();
    edit(&mut harness, 0, "[Unfinished");
    assert_eq!(raw(harness.state(), 0), Some("[Unfinished"));
    assert!(harness.state().has_unresolved_query_design_edits());
    assert_ne!(
        harness.state().query_design_request().as_ref(),
        Some(&old_request)
    );
    assert!(!harness.state_mut().begin_query_design_build(
        old_request,
        old_draft,
        QueryDesignCompletion::Run,
        101
    ));
    assert!(harness.state().query_design_run_action().is_none());
    assert!(harness.state_mut().show_query_design_sql().is_none());
    let request = harness.state().query_design_request().unwrap();
    assert!(
        !harness
            .state()
            .can_import_query_design(&request, &harness.state().sql)
    );

    edit(&mut harness, 1, "[Other heading]: CustomerID");
    assert_eq!(raw(harness.state(), 0), Some("[Unfinished"));
    assert_eq!(
        harness
            .state()
            .query_design
            .session
            .as_ref()
            .unwrap()
            .field_inputs
            .source_hint(0),
        None
    );
    let tab = harness.state().active_object().unwrap();
    assert!(!harness.state().can_close_object(&tab));
    assert!(!harness.state_mut().close_object(&tab));
    assert!(!harness.state_mut().close_object_confirmed(&tab));
    assert!(!harness.state().can_restart_for_update());
    harness.run_steps(2);
    assert_eq!(raw(harness.state(), 0), Some("[Unfinished"));
    // Close reveals the actual retained input; Escape remains reachable after
    // another column had focus and restores only this field's baseline.
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert_eq!(raw(harness.state(), 0), None);
    assert_eq!(
        field_cell_text(&harness.state().query_design_draft().unwrap().columns[0])
            .unwrap()
            .as_ref(),
        old_expression
    );
    assert_eq!(
        harness.state().query_design_draft().unwrap().columns[1]
            .alias
            .as_deref(),
        Some("Other heading")
    );
    assert!(!harness.state().has_unresolved_query_design_edits());
}

#[test]
fn parked_raw_text_survives_fresh_source_preferences_without_binding_hints() {
    let mut harness = harness(state());
    edit(&mut harness, 0, "CustomerCount: IIf('");
    let sql_tab = harness.state().active_object().unwrap();
    let revision = sql_tab.target.document_revision;
    let table_name = harness.state().query_design_draft().unwrap().sources[0]
        .table
        .as_str();
    // NO_CLONE: The table-open intent owns its selector while the SQL editor
    // and its metadata remain parked independently in the object registry.
    let table_name = table_name.to_owned();
    let table = harness
        .state_mut()
        .prepare_object_open(ObjectTarget {
            document_revision: revision,
            kind: ObjectKind::Table { name: table_name },
        })
        .unwrap()
        .tab;
    assert!(harness.state().has_unresolved_query_design_edits());
    assert!(!harness.state().can_close_object(&sql_tab));
    assert!(harness.state_mut().activate_object(&sql_tab));
    assert_eq!(raw(harness.state(), 0), Some("CustomerCount: IIf('"));
    assert!(harness.state_mut().activate_object(&table));
    assert!(harness.state().has_unresolved_query_design_edits());
    let preferences = {
        assert!(harness.state_mut().activate_object(&sql_tab));
        harness.state().query_design.preferences().unwrap()
    };
    let mut refreshed = super::super::QueryDesign::from_preferences(preferences);
    let session = refreshed.session.as_mut().unwrap();
    assert_eq!(session.field_inputs.text(0), Some("CustomerCount: IIf('"));
    assert_eq!(session.field_inputs.source_hint(0), None);
    assert!(session.metadata.is_empty());
    assert!(session.field_inputs.unresolved());
    assert!(refreshed.retained_bytes().unwrap() > size_of_val(&refreshed));
}

#[test]
fn retained_raw_and_cancel_baseline_participate_in_workspace_admission() {
    let mut state = state();
    let original_bytes = state.object_bytes().unwrap();
    let allowance = crate::workspace::objects::MAX_RETAINED_BYTES - original_bytes;
    assert!(state.admit_object_bytes(Some(allowance)));
    let session = state.query_design.session.as_mut().unwrap();
    let text = format!("Heading: IIf('{}", "x".repeat(8192));
    session
        .field_inputs
        .record(0, &session.draft.columns[0], text, false, None)
        .unwrap();
    session.revision += 1;
    assert!(!state.admit_object_bytes(Some(allowance)));
    assert!(state.has_unresolved_query_design_edits());
    assert!(state.query_design_run_action().is_none());
    assert!(!state.can_restart_for_update());
}

#[test]
fn cancel_reservation_cannot_be_spent_by_shortening_the_visible_operand() {
    let mut draft = QueryDesignDraft::default();
    let original = "x".repeat(MAX_DESIGN_TEXT_BYTES / 2);
    draft.columns.push(DesignColumn {
        expression: DesignExpression::Expression(original),
        ..DesignColumn::field(1, String::new())
    });
    let mut inputs = FieldInputs::default();
    let previous = draft_text_bytes(&draft).unwrap();
    inputs
        .record(0, &draft.columns[0], "1".into(), true, None)
        .unwrap();
    draft.columns[0].expression = DesignExpression::Expression("1".into());
    assert_eq!(inputs.editable_bytes(&draft).unwrap(), previous);
    assert!(inputs.cancel(0, &mut draft.columns[0]).unwrap());
    assert_eq!(draft_text_bytes(&draft).unwrap(), previous);
    assert!(!inputs.unresolved());

    // A subsequent complete edit may grow the parsed model beyond the Cancel
    // baseline. A shorter incomplete shadow must reserve that current model too.
    let larger = "y".repeat(MAX_DESIGN_TEXT_BYTES * 3 / 4);
    // NO_CLONE: The retained raw input and editable structured model are
    // independent owners whose accounting must remain correct after divergence.
    inputs
        .record(0, &draft.columns[0], larger.clone(), true, None)
        .unwrap();
    draft.columns[0].expression = DesignExpression::Expression(larger);
    let current = draft_text_bytes(&draft).unwrap();
    inputs
        .record(0, &draft.columns[0], "[".into(), false, None)
        .unwrap();
    assert_eq!(inputs.editable_bytes(&draft).unwrap(), current);
    assert!(inputs.unresolved());
}
