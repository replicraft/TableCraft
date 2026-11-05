use super::*;
use crate::workspace::Action;
use crate::{
    Data, Database, FieldFilterOperator as Op, FilterPublication, FilterTarget, RowOrdinals, Table,
};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn state() -> (State, FieldFilterTarget) {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Field filters".into(),
        format: "ACE16".into(),
        tables: vec![Table {
            local: true,
            name: "Items".into(),
            system: false,
            storage: "Local".into(),
        }],
        queries: Vec::new(),
    });
    state
        .prepare_object_open(crate::ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Table {
                name: "Items".into(),
            },
        })
        .unwrap();
    assert!(state.set_table_data(Data {
        row_ordinals: None,
        source_revision: Some(7),
        table: "Items".into(),
        offset: 0,
        columns: vec!["Field".into()],
        rows: vec![vec!["12".into()]],
        has_more: false,
    }));
    let target = state.field_filter_target("Field").unwrap();
    (state, target)
}

fn description(target: FieldFilterTarget) -> FieldFilterDescription {
    FieldFilterDescription {
        target,
        input_hint: InputHint::Decimal,
        nullable: true,
        operators: vec![Op::Equal, Op::Between, Op::IsNull],
        pattern_mode: None,
    }
}

#[test]
fn metadata_and_apply_bind_exact_draft_and_failure_preserves_inputs_and_previous_page() {
    let (mut state, target) = state();
    let rows = state.data.as_ref().unwrap().rows.as_ptr();
    assert!(state.begin_field_filter(target.clone(), 1));
    assert!(!state.set_field_filter_description(9, description(target.clone())));
    assert!(state.set_field_filter_description(1, description(target.clone())));
    let field = state.table_filter.field.as_mut().unwrap();
    field.operator = Op::Between;
    field.inputs = ["10".into(), "5".into()];
    let inputs = vec!["10".into(), "5".into()];
    assert!(!state.can_apply_field_filter(&target, Op::Equal, &inputs));
    assert!(state.can_apply_field_filter(&target, Op::Between, &inputs));
    assert!(state.begin_field_filter_apply(2, target.clone(), Op::Between, inputs.clone()));
    state.fail_filter(2, "Literal native validation diagnosis".into());
    let field = state.table_filter.field.as_ref().unwrap();
    assert!(!field.applying);
    assert_eq!(field.inputs(), inputs);
    assert_eq!(field.operator, Op::Between);
    assert_eq!(
        field.error.as_deref(),
        Some("Literal native validation diagnosis")
    );
    assert_eq!(state.data.as_ref().unwrap().rows.as_ptr(), rows);
    assert!(state.can_apply_field_filter(&target, Op::Between, &inputs));
    assert!(state.begin_field_filter_apply(3, target.clone(), Op::Between, inputs.clone()));
    assert!(!state.set_filtered_data(
        3,
        FilterPublication {
            target: FilterTarget {
                document_revision: 7,
                table: "Items".into(),
                filter_id: 99
            },
            sort: None,
            filter_count: 1,
            enabled: true,
            matched_rows: 0,
            source_rows: 1,
            data: Data {
                row_ordinals: Some(RowOrdinals::new(Vec::new()).unwrap()),
                source_revision: Some(7),
                table: "Wrong table".into(),
                offset: 0,
                columns: vec!["Field".into()],
                rows: Vec::new(),
                has_more: false,
            },
        }
    ));
    let field = state.table_filter.field.as_ref().unwrap();
    assert!(
        !field.applying,
        "malformed envelope restores editable draft"
    );
    assert_eq!(field.inputs(), inputs);
    assert!(field.error.is_some());
    assert_eq!(state.data.as_ref().unwrap().rows.as_ptr(), rows);
    state.table_filter.field.as_mut().unwrap().inputs[0] = "é".repeat(MAX_INPUT_BYTES / 2 + 1);
    assert!(!state.can_apply_field_filter(
        &target,
        Op::Between,
        &state.table_filter.field.as_ref().unwrap().inputs()
    ));
    assert!(state.close_field_filter(3));
    assert!(!state.set_field_filter_description(1, description(target.clone())));
    assert!(state.begin_field_filter(target.clone(), 4));
    let page = state.data.take().unwrap();
    assert!(state.set_table_data(page));
    assert!(!state.set_field_filter_description(4, description(target.clone())));
    assert!(!state.field_filter_dialog_open());
    assert!(state.begin_field_filter(target.clone(), 5));
    state.fail_field_filter(5, "Metadata request already failed".into());
    assert!(!state.set_field_filter_description(5, description(target)));
    state.fail_field_filter(5, "Duplicate malformed metadata response".into());
    assert!(
        state
            .table_filter
            .field
            .as_ref()
            .unwrap()
            .description
            .is_none()
    );
    assert_eq!(
        state.table_filter.field.as_ref().unwrap().error.as_deref(),
        Some("Metadata request already failed")
    );
}

#[test]
fn actual_dialog_emits_entered_range_and_escape_closes_only_after_operator_popup() {
    let (mut state, target) = state();
    assert!(state.begin_field_filter(target.clone(), 1));
    assert!(state.set_field_filter_description(1, description(target.clone())));
    let tab = state.active_object().unwrap();
    assert!(!state.can_activate_object(&tab));
    assert!(!state.can_close_object(&tab));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness
        .get_by_role_and_label(Role::ComboBox, "Comparison")
        .click();
    harness.run();
    harness.get_by_label("Between").click();
    harness.run();
    for (label, text) in [("Value", "10"), ("Second value", "5")] {
        harness
            .get_by_role_and_label(Role::MultilineTextInput, label)
            .focus();
        harness.run();
        harness
            .get_by_role_and_label(Role::MultilineTextInput, label)
            .type_text(text);
        harness.run();
    }
    let apply = harness.get_by_role_and_label(Role::Button, "Apply filter");
    let cancel = harness.get_by_role_and_label(Role::Button, "Cancel");
    assert!(apply.rect().left() < cancel.rect().left());
    assert!(apply.rect().bottom() <= 580.0 && cancel.rect().bottom() <= 580.0);
    apply.click();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::ApplyFieldFilter { target: actual, operator: Op::Between, inputs }]
        if actual == &target && inputs == &["10", "5"])
    );
    harness.state_mut().1.clear();
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Value")
        .focus();
    harness.run();
    harness.key_press(egui::Key::End);
    harness.run();
    for incoming in [
        "é".repeat(MAX_INPUT_BYTES / 2 + 1),
        "x".repeat(MAX_INPUT_BYTES),
    ] {
        harness.event(egui::Event::Paste(incoming));
        harness.run();
        assert_eq!(
            harness
                .state()
                .0
                .table_filter
                .field
                .as_ref()
                .unwrap()
                .inputs[0],
            "10"
        );
        assert!(
            harness
                .state()
                .0
                .table_filter
                .field
                .as_ref()
                .unwrap()
                .error
                .is_some()
        );
        assert!(harness.state().1.is_empty());
    }
    for replacement in ["x".repeat(MAX_INPUT_BYTES), "12".into()] {
        harness.event(egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        });
        harness.run();
        harness.event(egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        });
        harness.run();
        harness.event(egui::Event::Paste(replacement.clone()));
        harness.run();
        assert_eq!(
            harness
                .state()
                .0
                .table_filter
                .field
                .as_ref()
                .unwrap()
                .inputs[0],
            replacement
        );
        assert!(harness.state().1.is_empty());
    }
    harness
        .get_by_role_and_label(Role::ComboBox, "Comparison")
        .click();
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.state().0.field_filter_dialog_open());
    assert!(harness.state().1.is_empty());
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::CloseFieldFilter { request_id: 1 }]
    ));
    assert_eq!(
        harness.state().0.data.as_ref().unwrap().rows[0][0].text,
        "12"
    );
}

#[test]
fn home_and_empty_table_header_emit_exact_metadata_requests_without_reading_a_row() {
    let (state, target) = state();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 740.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Filter")
        .click();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::DescribeFieldFilter { target: actual }] if actual == &target)
    );
    harness.state_mut().1.clear();
    let state = &mut harness.state_mut().0;
    let mut data = state.data.take().unwrap();
    data.rows.clear();
    assert!(state.set_table_data(data));
    harness.run();
    assert!(harness.state().0.current_field_filter_target().is_none());
    let header = harness
        .get_all_by_role(Role::Button)
        .find(|node| {
            node.accesskit_node().label().is_some_and(|label| {
                label.replace(['\u{2068}', '\u{2069}'], "") == "Column menu Field"
            })
        })
        .expect("actual native header dropdown");
    header.click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Field filter")
        .click();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::DescribeFieldFilter { target: actual }] if actual == &target)
    );
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn cursor_movement_before_paste_cannot_use_a_stale_selection_to_admit_a_prefix() {
    let (mut state, target) = state();
    assert!(state.begin_field_filter(target.clone(), 1));
    assert!(state.set_field_filter_description(1, description(target)));
    let near_limit = "x".repeat(MAX_INPUT_BYTES - 1);
    state.table_filter.field.as_mut().unwrap().inputs[0] = near_limit.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Value")
        .focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    let focused = harness.ctx.memory(|memory| memory.focused()).unwrap();
    let selection = egui::text_edit::TextEditState::load(&harness.ctx, focused)
        .unwrap()
        .cursor
        .char_range()
        .unwrap()
        .as_sorted_char_range();
    assert_eq!(usize::from(selection.start), 0);
    assert_eq!(usize::from(selection.end), MAX_INPUT_BYTES - 1);
    harness.input_mut().events.push(egui::Event::Key {
        key: egui::Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness
        .input_mut()
        .events
        .push(egui::Event::Paste("XY".into()));
    harness.run();
    assert_eq!(
        harness
            .state()
            .0
            .table_filter
            .field
            .as_ref()
            .unwrap()
            .inputs[0],
        near_limit,
        "collapsing a selection before an oversized paste must not admit a prefix"
    );
    assert!(
        harness
            .state()
            .0
            .table_filter
            .field
            .as_ref()
            .unwrap()
            .error
            .is_some(),
        "the whole insertion must be rejected visibly"
    );
    assert!(harness.state().1.is_empty());
}
