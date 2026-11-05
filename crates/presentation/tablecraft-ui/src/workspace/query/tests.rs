use super::*;
use crate::{Database, ParameterSpec, SavedQuery, SavedQueryTarget};
use egui::{Key, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn state() -> State {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Keyboard query review".into(),
        format: "ACE 16".into(),
        tables: Vec::new(),
        queries: vec![SavedQuery {
            object_id: 9,
            name: "Typed prompts".into(),
            system: false,
        }],
    });
    state.select_view(DocumentView::Query);
    state
}

fn harness(state: State) -> Harness<'static, (State, Vec<Action>)> {
    Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        )
}

#[test]
fn long_sql_remains_bounded_with_existing_run_controls_and_literal_text() {
    let mut state = state();
    state.sql = "SELECT 1\n".repeat(80);
    let original = state.sql.clone();
    let mut harness = harness(state);
    harness.run();
    let editor = harness.get_by_role_and_label(Role::MultilineTextInput, "SQL query");
    // Egui nests the editor through several generic accessibility containers;
    // its scrollbar is owned by an enclosing scope, not the text-input node.
    let mut scope = editor.parent();
    let viewport = loop {
        let parent = scope.expect("an enclosing SQL scroll scope exists");
        let vertical: Vec<_> = parent
            .children_recursive()
            .filter(|node| node.accesskit_node().role() == Role::ScrollBar)
            .map(|bar| bar.rect())
            .filter(|rect| rect.height() > rect.width())
            .collect();
        match vertical.as_slice() {
            [viewport] => break *viewport,
            [] => scope = parent.parent(),
            _ => panic!("the nearest SQL scroll scope must have one vertical scrollbar"),
        }
    };
    assert!(viewport.height() <= MAX_SQL_EDITOR_HEIGHT);
    let text_clip = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == original => Some(shape.clip_rect),
            _ => None,
        })
        .expect("actual SQL is painted through the bounded editor viewport");
    assert!(text_clip.height() <= MAX_SQL_EDITOR_HEIGHT + 2.0);
    let run = harness.get_by_role_and_label(Role::Button, "Run query");
    assert!(run.rect().bottom() < 550.0);
    editor.focus();
    harness.run();
    assert_eq!(harness.state().0.sql, original);
    assert!(harness.state().1.is_empty());
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::Enter);
    harness.run();
    assert!(matches!(harness.state().1.as_slice(), [Action::Query]));
}

#[test]
fn sequential_prompt_tab_order_preserves_all_inputs_and_dispatches_only_after_last_ok() {
    let mut state = state();
    let target = SavedQueryTarget {
        document_revision: 7,
        object_id: 9,
        name: "Typed prompts".into(),
    };
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(
        state.set_saved_query_details(
            1,
            target,
            (1..=12)
                .map(|index| ParameterSpec {
                    name: format!("Prompt {index:02}"),
                    kind: "int32".into(),
                    label: "LONG".into(),
                    nullable: true,
                })
                .collect()
        )
    );
    let mut harness = harness(state);
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click();
    harness.run();
    for ordinal in 1..=12 {
        let label = format!("Prompt {ordinal:02}");
        let input = harness.get_by_role_and_label(Role::TextInput, &label);
        assert!(input.is_focused());
        harness.key_press(Key::Tab);
        harness.run();
        let null = harness
            .get_all_by_role(Role::CheckBox)
            .find(|control| {
                control
                    .accesskit_node()
                    .labelled_by()
                    .any(|prompt| prompt.value().as_deref() == Some(label.as_str()))
            })
            .expect("the nullable control belongs to the current prompt");
        assert!(null.is_focused());
        assert!(harness.state().1.is_empty());
        harness.get_by_role_and_label(Role::Button, "OK").click();
        harness.run();
    }
    assert!(!harness.state().0.saved_query_prompt_open());
    assert!(
        matches!(harness.state().1.as_slice(), [Action::RunSavedQuery { parameters, .. }] if parameters.len() == 12)
    );
}
