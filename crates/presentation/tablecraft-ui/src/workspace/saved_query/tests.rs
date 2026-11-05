use super::*;
use crate::{Database, SavedQuery};

fn state() -> (State, SavedQueryTarget) {
    let target = SavedQueryTarget {
        document_revision: 7,
        object_id: 90,
        name: "Orders by minimum".into(),
    };
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        document_revision: 7,
        title: "Example".into(),
        format: "ACE16".into(),
        tables: vec![],
        queries: vec![SavedQuery {
            object_id: 90,
            name: target.name.clone(),
            system: false,
        }],
    });
    state.sql = "SELECT 42".into();
    state.set_query_data(data("SQL result", "42"));
    (state, target)
}

fn data(name: &str, text: &str) -> Data {
    Data {
        source_revision: None,
        table: name.into(),
        offset: 0,
        columns: vec!["Result".into()],
        rows: vec![vec![text.into()]],
        has_more: false,
    }
}

fn specs() -> Vec<ParameterSpec> {
    vec![
        ParameterSpec {
            name: "Minimum".into(),
            kind: "int32".into(),
            label: "LONG".into(),
            nullable: true,
        },
        ParameterSpec {
            name: "Choice".into(),
            kind: "value".into(),
            label: "VALUE".into(),
            nullable: true,
        },
    ]
}

#[test]
fn inspected_parameters_require_explicit_value_type_and_preserve_sql_result_slot() {
    let (mut state, target) = state();
    assert!(state.can_inspect_saved_query(&target));
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(!state.set_saved_query_details(9, target.clone(), specs()));
    assert!(state.set_saved_query_details(1, target.clone(), specs()));
    state.saved_query.inputs[0].text = "7".into();
    state.saved_query.inputs[1].is_null = true;
    assert!(!state.can_run_saved_query(&target, &state.saved_query.inputs));
    state.saved_query.inputs[1].kind = "text".into();
    assert!(state.can_run_saved_query(&target, &state.saved_query.inputs));
    let inputs = state.saved_query.inputs.clone();
    state.begin_saved_query_run(target.clone(), inputs, 2);
    let result = data("Saved result", "7");
    assert!(state.set_saved_query_data(2, target, result));
    assert!(
        state
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .edit_target(0, 0)
            .is_none()
    );
    assert!(
        state
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .source_revision
            .is_none()
    );
    assert_eq!(state.visible_data().unwrap().rows[0][0].text, "7");
    state.saved_query.active = false;
    assert_eq!(state.visible_data().unwrap().rows[0][0].text, "42");
    assert_eq!(state.sql, "SELECT 42");
}

#[test]
fn source_object_request_and_input_binding_reject_stale_results_and_errors() {
    let (mut state, target) = state();
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(state.set_saved_query_details(1, target.clone(), vec![]));
    state.begin_saved_query_run(target.clone(), vec![], 2);
    let mut wrong = target.clone();
    wrong.object_id += 1;
    assert!(!state.set_saved_query_data(2, wrong.clone(), data("Wrong", "bad")));
    state.fail_saved_query(2, wrong, "Wrong object".into());
    assert!(state.saved_query.error.is_none());
    state.select_view(DocumentView::Table);
    assert!(!state.set_saved_query_data(2, target.clone(), data("Late", "bad")));
    state.fail_saved_query(2, target.clone(), "Late failure".into());
    assert!(state.view == DocumentView::Table);
    assert!(
        !state
            .saved_query
            .error
            .as_deref()
            .is_some_and(|error| error == "Late failure")
    );
    state.clear_schema();
    assert!(state.selected_saved_target().is_none());
    target_revision_refused(&state, target);
}

fn target_revision_refused(state: &State, mut target: SavedQueryTarget) {
    target.document_revision += 1;
    assert!(!state.can_inspect_saved_query(&target));
}

#[test]
fn typed_inputs_are_bound_and_limited_before_any_action_clone_including_null_text() {
    let (mut state, target) = state();
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(state.set_saved_query_details(
        1,
        target.clone(),
        vec![ParameterSpec {
            name: "P".into(),
            kind: "text".into(),
            label: "TEXT".into(),
            nullable: true,
        }]
    ));
    state.saved_query.inputs[0].text = "x".repeat(MAX_INPUT_BYTES - 5);
    assert!(state.can_run_saved_query(&target, &state.saved_query.inputs));
    state.saved_query.inputs[0].is_null = true;
    state.saved_query.inputs[0].text.push('é');
    assert!(!state.can_run_saved_query(&target, &state.saved_query.inputs));
    assert!(state.saved_query_run_action().is_none());
    state.saved_query.inputs[0].text.clear();
    let mut stale = state.saved_query.inputs.clone();
    stale[0].text = "different input".into();
    assert!(!state.can_run_saved_query(&target, &stale));

    let submitted = state.saved_query.inputs.clone();
    state.begin_saved_query_run(target.clone(), submitted, 2);
    state.saved_query.inputs[0].text = "newer input".into();
    assert!(!state.set_saved_query_data(2, target.clone(), data("Stale", "bad")));
    state.fail_saved_query(2, target, "Failure for previous input".into());
    assert!(state.saved_query.error.is_none());
    assert!(state.saved_query.data.is_none());
}

#[test]
fn object_click_inspects_only_and_explicit_run_emits_exact_typed_parameters() {
    let (mut state, target) = state();
    let context = egui::Context::default();
    let actions = click(&mut state, &context, &target.name);
    assert!(
        matches!(actions.as_slice(), [Action::InspectSavedQuery { target: actual }] if actual == &target)
    );
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(render(&mut state, &context, vec![]).0.is_empty());
    assert!(state.set_saved_query_details(1, target.clone(), specs()));
    state.saved_query.inputs[0].text = "7".into();
    assert!(click(&mut state, &context, "Choose a value type").is_empty());
    assert!(click(&mut state, &context, "Text").is_empty());
    assert_eq!(state.saved_query.inputs[1].kind, "text");
    state.saved_query.inputs[1].text = "literal 'text'".into();
    let actions = click(&mut state, &context, "Run saved query");
    assert!(
        matches!(actions.as_slice(), [Action::RunSavedQuery { target: actual, parameters }]
        if actual == &target && parameters[0].kind == "int32" && parameters[0].text == "7"
            && parameters[1].kind == "text" && parameters[1].text == "literal 'text'")
    );
    assert_eq!(state.sql, "SELECT 42");
}

fn render(
    state: &mut State,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> (Vec<Action>, egui::FullOutput) {
    let mut actions = Vec::new();
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100.0, 740.0),
            )),
            events,
            ..Default::default()
        },
        |ui| actions = state.show(ui),
    );
    output.textures_delta.clear();
    (actions, output)
}

fn click(state: &mut State, context: &egui::Context, label: &str) -> Vec<Action> {
    render(state, context, vec![]);
    let (_, output) = render(state, context, vec![]);
    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("actual saved-query command label");
    let mut actions = Vec::new();
    for pressed in [true, false] {
        actions.extend(
            render(
                state,
                context,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            )
            .0,
        );
    }
    actions
}

#[test]
fn replaced_saved_result_surrenders_old_grid_focus_and_shortcuts_follow_visible_query() {
    let (mut state, target) = state();
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(state.set_saved_query_details(1, target.clone(), vec![]));
    state.begin_saved_query_run(target.clone(), vec![], 2);
    let mut old = data("Saved result", "Old first");
    old.rows.push(vec!["Old second".into()]);
    assert!(state.set_saved_query_data(2, target.clone(), old));
    let context = egui::Context::default();
    assert!(click(&mut state, &context, "Old second").is_empty());
    let old_focus = context
        .memory(|memory| memory.focused())
        .expect("actual clicked grid cell");
    assert_eq!(state.saved_query.selection.position(), Some((1, 0)));

    // Complete the new inspection/run before a paint frame, retaining the same
    // result name/offset so stale widget focus cannot be hidden by a new ID.
    state.begin_saved_query_inspection(target.clone(), 3);
    assert!(state.set_saved_query_details(3, target.clone(), vec![]));
    state.begin_saved_query_run(target.clone(), vec![], 4);
    let mut replacement = data("Saved result", "NEW first");
    replacement.rows.push(vec!["NEW second".into()]);
    assert!(state.set_saved_query_data(4, target.clone(), replacement));
    assert!(render(&mut state, &context, vec![]).0.is_empty());
    assert_ne!(context.memory(|memory| memory.focused()), Some(old_focus));
    assert_eq!(state.selected_cell_text(), Some("NEW first"));
    assert!(
        render(&mut state, &context, vec![egui::Event::Copy])
            .0
            .is_empty()
    );
    assert!(
        state
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .edit_target(0, 0)
            .is_none()
    );
    assert!(
        state
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .contents_target(0, 0)
            .is_none()
    );

    let command = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let actions = render(&mut state, &context, vec![enter(true, command)]).0;
    assert!(
        matches!(actions.as_slice(), [Action::RunSavedQuery { target: actual, parameters }]
        if actual == &target && parameters.is_empty())
    );
    assert!(
        render(&mut state, &context, vec![enter(false, command)])
            .0
            .is_empty()
    );
    assert!(click(&mut state, &context, "SQL query").is_empty());
    let actions = render(&mut state, &context, vec![enter(true, command)]).0;
    assert!(matches!(actions.as_slice(), [Action::Query]));
    assert_eq!(state.visible_data().unwrap().rows[0][0].text, "42");
}

fn enter(pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

#[test]
fn wrong_saved_result_envelopes_preserve_previous_results_and_allow_a_new_run() {
    let (mut state, target) = state();
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(state.set_saved_query_details(1, target.clone(), vec![]));
    state.begin_saved_query_run(target.clone(), vec![], 2);
    assert!(state.set_saved_query_data(2, target.clone(), data("Saved result", "original")));

    for (index, invalid) in ["native page", "offset", "partial result"]
        .iter()
        .enumerate()
    {
        let request_id = 3 + index as u64;
        state.begin_saved_query_run(target.clone(), vec![], request_id);
        let mut wrong = data("Wrong result", "must not publish");
        match *invalid {
            "native page" => wrong.source_revision = Some(7),
            "offset" => wrong.offset = 1,
            "partial result" => wrong.has_more = true,
            _ => unreachable!(),
        }
        assert!(!state.set_saved_query_data(request_id, target.clone(), wrong));
        assert_eq!(state.visible_data().unwrap().rows[0][0].text, "original");
        assert!(state.saved_query.pending.is_none());
        assert!(
            state
                .saved_query
                .error
                .as_ref()
                .unwrap()
                .contains("complete saved-query result")
        );
        assert!(state.can_run_saved_query(&target, &[]));
    }

    state.begin_saved_query_run(target.clone(), vec![], 9);
    assert!(state.set_saved_query_data(9, target, data("Saved result", "new successful result")));
    assert_eq!(
        state.visible_data().unwrap().rows[0][0].text,
        "new successful result"
    );
    assert!(state.saved_query.error.is_none());
}

#[test]
fn parameter_type_and_null_controls_reference_their_actual_prompt_labels() {
    use egui::accesskit::Role;

    let (mut state, target) = state();
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(state.set_saved_query_details(1, target, specs()));
    let context = egui::Context::default();
    context.enable_accesskit();
    let (actions, output) = render(&mut state, &context, vec![]);
    assert!(actions.is_empty());
    let tree = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("actual accessibility tree");
    let prompt_id = |text| {
        tree.nodes
            .iter()
            .find_map(|(id, node)| {
                (node.role() == Role::Label
                    && node
                        .value()
                        .is_some_and(|value| value.replace(['\u{2068}', '\u{2069}'], "") == text))
                .then_some(*id)
            })
            .expect("actual visible parameter prompt")
    };
    let choice = prompt_id("Choice · VALUE");
    assert!(tree.nodes.iter().any(|(_, node)| {
        node.role() == Role::ComboBox
            && node.labelled_by().contains(&choice)
            && node.description() == Some("Parameter value type")
    }));
    for prompt in ["Minimum · LONG", "Choice · VALUE"] {
        let id = prompt_id(prompt);
        assert_eq!(
            tree.nodes
                .iter()
                .filter(|(_, node)| {
                    node.role() == Role::CheckBox
                        && node.labelled_by().contains(&id)
                        && node.description() == Some("Use NULL for this parameter")
                })
                .count(),
            1,
            "NULL control for {prompt}"
        );
    }
}
