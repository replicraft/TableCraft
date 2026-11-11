use super::*;
use crate::{Database, SavedQuery, SavedQueryTarget};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn page(offset: u32, count: usize, has_more: bool) -> Data {
    Data {
        query_contents: None,
        row_ordinals: None,
        source_revision: Some(7),
        table: "Items".into(),
        offset,
        columns: vec!["ID".into(), "Name".into()],
        rows: (0..count)
            .map(|row| {
                vec![
                    (u64::from(offset) + row as u64).to_string().into(),
                    "value".into(),
                ]
            })
            .collect(),
        has_more,
    }
}

fn state() -> State {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Example".into(),
        format: "ACE16".into(),
        queries: vec![],
        tables: vec![],
    });
    state.set_table_data(page(100, 100, true));
    state
}

#[test]
fn navigation_keeps_column_and_accepts_only_matching_source_request_and_record() {
    let mut state = state();
    state.data.as_mut().unwrap().columns[1].caption = Some("Friendly name".into());
    state
        .table_selection
        .select(state.data.as_ref().unwrap(), 99, 1);
    let mut target = state.selected_record_target().unwrap();
    assert_eq!(target.parent_ordinal, 199);
    target.parent_ordinal = 200;
    let mut actions = Vec::new();
    state.request_record(target.clone(), &mut actions);
    assert!(
        matches!(actions.as_slice(), [Action::NavigateRecord { target }] if target.parent_ordinal == 200 && target.column == "Name")
    );
    state.begin_record_navigation(target.clone(), 10);
    assert_eq!(state.table_selection.position(), Some((99, 1)));
    assert!(!state.set_record_data(9, &target, page(200, 3, false)));
    assert!(!state.set_record_data(10, &target, page(199, 4, false)));
    assert_eq!(state.data.as_ref().unwrap().offset, 100);
    assert_eq!(state.table_selection.position(), Some((99, 1)));
    assert!(
        state
            .records
            .error
            .as_ref()
            .unwrap()
            .contains("another record position")
    );
    state.begin_record_navigation(target.clone(), 11);
    assert!(state.set_record_data(11, &target, page(200, 3, false)));
    assert_eq!(state.table_selection.position(), Some((0, 1)));
    assert_eq!(state.data.as_ref().unwrap().offset, 200);
}

#[test]
fn checked_eof_failure_and_replaced_view_preserve_the_previous_page() {
    let mut state = state();
    let mut target = state.selected_record_target().unwrap();
    target.parent_ordinal = 999;
    state.begin_record_navigation(target.clone(), 10);
    assert!(!state.set_record_data(10, &target, page(999, 0, false)));
    assert_eq!(state.data.as_ref().unwrap().offset, 100);
    assert!(state.records.error.as_ref().unwrap().contains("no record"));

    state.begin_record_navigation(target.clone(), 11);
    state.fail_record_navigation(11, "Read cancelled".into());
    assert_eq!(state.data.as_ref().unwrap().offset, 100);
    assert_eq!(state.records.error.as_deref(), Some("Read cancelled"));

    state.begin_record_navigation(target.clone(), 12);
    state.select_view(DocumentView::Query);
    assert!(!state.set_record_data(12, &target, page(999, 1, false)));
    assert_eq!(state.data.as_ref().unwrap().offset, 100);
    assert!(state.view == DocumentView::Query);
}

#[test]
fn loaded_navigation_has_no_read_action_and_record_numbers_are_checked() {
    let mut state = state();
    let mut target = state.selected_record_target().unwrap();
    target.parent_ordinal = 101;
    let mut actions = Vec::new();
    state.request_record(target, &mut actions);
    assert!(actions.is_empty());
    assert_eq!(state.table_selection.position(), Some((1, 0)));
    assert_eq!(parse_record("1"), Some(0));
    assert_eq!(parse_record("4294967296"), Some(u32::MAX));
    for invalid in ["0", "-1", "1.5", "4294967297", ""] {
        assert_eq!(parse_record(invalid), None);
    }
    state.show_open();
    assert!(state.selected_record_target().is_none());
    state.dismiss_backstage();
    state.data.as_mut().unwrap().source_revision = None;
    assert!(state.selected_record_target().is_none());
}

#[test]
fn actual_navigator_previous_button_requests_global_record_without_replacing_data() {
    let mut harness = Harness::builder().build_ui_state(
        |ui, state: &mut (State, Vec<Action>)| state.0.record_navigator(ui, &mut state.1),
        (state(), Vec::new()),
    );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Previous")
        .click_accesskit();
    harness.run_steps(2);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::NavigateRecord { target }] if target.parent_ordinal == 99 && target.document_revision == 7 && target.table == "Items")
    );
    assert_eq!(harness.state().0.data.as_ref().unwrap().offset, 100);
    assert_eq!(harness.state().0.table_selection.position(), Some((0, 0)));
}

#[test]
fn record_navigator_remains_visible_below_the_virtual_datasheet() {
    let mut state = state();
    let context = egui::Context::default();
    for size in [egui::vec2(900.0, 580.0), egui::vec2(1280.0, 820.0)] {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            },
            |ui| {
                assert!(state.show(ui).is_empty());
            },
        );
        output.textures_delta.clear();
        let navigator = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == "Record:" => {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
                }
                _ => None,
            })
            .expect("actual bottom record navigator");
        assert!(
            navigator.top() > size.y - 100.0,
            "{navigator:?}, viewport {size:?}"
        );
        assert!(
            navigator.bottom() < size.y,
            "{navigator:?}, viewport {size:?}"
        );
        let mut visible_values = 0;
        for shape in &output.shapes {
            if let egui::epaint::Shape::Text(text) = &shape.shape
                && text.galley.job.text == "value"
            {
                let rect = egui::Rect::from_min_size(text.pos, text.galley.size())
                    .intersect(shape.clip_rect);
                if rect.is_positive() {
                    visible_values += 1;
                    assert!(
                        rect.bottom() < navigator.top(),
                        "cell {rect:?}, navigator {navigator:?}"
                    );
                }
            }
        }
        assert!(visible_values > 0);
    }
}

#[test]
fn focused_record_input_plain_enter_submits_once_but_modified_enter_does_not() {
    let mut initial = state();
    initial.sql = "SELECT 1".into();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 740.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (initial, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Record number")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Record number")
        .type_text("301");
    harness.run_steps(1);
    assert_eq!(harness.state().0.records.position, "301");
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::NavigateRecord { target }]
        if target.document_revision == 7 && target.table == "Items"
            && target.parent_ordinal == 300 && target.column == "ID")
    );
    harness.run_steps(2);
    assert_eq!(harness.state().1.len(), 1);
    assert_eq!(harness.state().0.data.as_ref().unwrap().offset, 100);
    harness.state_mut().1.clear();
    for modifiers in [
        egui::Modifiers {
            shift: true,
            ..Default::default()
        },
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
    ] {
        harness
            .get_by_role_and_label(Role::TextInput, "Record number")
            .focus();
        harness.run_steps(1);
        harness.key_press_modifiers(modifiers, egui::Key::Enter);
        harness.run_steps(2);
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn ribbon_go_to_reuses_checked_navigation_and_enables_last_only_at_checked_eof() {
    let mut harness = Harness::builder().build_ui_state(
        |ui, state: &mut (State, Vec<Action>)| state.0.record_menu(ui, &mut state.1),
        (state(), Vec::new()),
    );
    crate::configure(&harness.ctx);
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Go To")
        .click_accesskit();
    harness.run();
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Last")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_by_role_and_label(Role::Button, "Next")
        .click_accesskit();
    harness.run();
    assert_eq!(harness.state().0.table_selection.position(), Some((1, 0)));
    assert!(harness.state().1.is_empty());

    harness.state_mut().0.data.as_mut().unwrap().has_more = false;
    harness
        .get_by_role_and_label(Role::Button, "Go To")
        .click_accesskit();
    harness.run();
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Last")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_by_role_and_label(Role::Button, "Last")
        .click_accesskit();
    harness.run();
    assert_eq!(harness.state().0.table_selection.position(), Some((99, 0)));
    assert!(harness.state().1.is_empty());

    harness
        .get_by_role_and_label(Role::Button, "Go To")
        .click_accesskit();
    harness.run();
    let input = harness.get_by_role(Role::TextInput);
    input.focus();
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("0");
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Go")
        .click_accesskit();
    harness.run();
    assert!(harness.state().1.is_empty());
    assert!(harness.state().0.records.error.is_some());
    harness.state_mut().0.records.input = "201".into();
    harness
        .get_by_role_and_label(Role::Button, "Go")
        .click_accesskit();
    harness.run();
    assert!(matches!(harness.state().1.as_slice(),
        [Action::NavigateRecord { target }]
            if target.document_revision == 7 && target.table == "Items"
                && target.parent_ordinal == 200 && target.column == "ID"
    ));
    assert_eq!(harness.state().0.data.as_ref().unwrap().offset, 100);
}

#[test]
fn home_select_record_preserves_field_and_copies_one_complete_loaded_record() {
    let mut state = state();
    state
        .table_selection
        .select(state.data.as_ref().unwrap(), 3, 1);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Select")
        .click_accesskit();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Select record")
        .click_accesskit();
    harness.run();
    assert_eq!(harness.state().0.table_selection.position(), Some((3, 1)));
    assert!(harness.state().0.table_selection.row_selected());
    assert!(harness.state().1.is_empty());
    let context = harness.ctx.clone();
    assert!(harness.state_mut().0.copy_selected(&context));
    let mut output = context.run_ui(Default::default(), |_| {});
    output.textures_delta.clear();
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText("103\tvalue\n".into()),)
    );

    harness.state_mut().0.busy = true;
    assert!(!harness.state().0.can_select_record());
    harness.state_mut().0.busy = false;
    harness.state_mut().0.view = DocumentView::Query;
    assert!(!harness.state().0.can_select_record());
    harness.state_mut().0.view = DocumentView::Table;
    harness.state_mut().0.data.as_mut().unwrap().source_revision = None;
    assert!(!harness.state().0.can_select_record());
}

#[test]
fn home_select_record_uses_visible_query_and_saved_result_without_native_handles() {
    for saved in [false, true] {
        let mut state = state();
        let mut result = page(0, 2, false);
        result.source_revision = None;
        result.table = "Read-only result".into();
        if saved {
            let target = SavedQueryTarget {
                document_revision: 7,
                object_id: 90,
                name: "Read-only saved result".into(),
            };
            state.database.as_mut().unwrap().queries.push(SavedQuery {
                object_id: target.object_id,
                name: target.name.clone(),
                system: false,
            });
            state.begin_saved_query_inspection(target.clone(), 1);
            assert!(state.set_saved_query_details(1, &target, Vec::new()));
            state.begin_saved_query_run(&target, Vec::new(), 2);
            assert!(state.set_saved_query_data(2, &target, result));
            state
                .saved_query
                .selection
                .select(state.saved_query.data.as_ref().unwrap(), 1, 1);
        } else {
            state.set_query_data(None, result);
            state
                .query_selection
                .select(state.query_data.as_ref().unwrap(), 1, 1);
        }
        state.view = DocumentView::Query;
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 820.0))
            .build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                (state, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run();
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Go To")
                .accesskit_node()
                .is_disabled()
        );
        harness
            .get_by_role_and_label(Role::Button, "Select")
            .click_accesskit();
        harness.run();
        harness
            .get_by_role_and_label(Role::Button, "Select record")
            .click_accesskit();
        harness.run();
        let state = &harness.state().0;
        let selection = if saved {
            &state.saved_query.selection
        } else {
            &state.query_selection
        };
        assert_eq!(selection.position(), Some((1, 1)));
        assert!(selection.row_selected());
        assert!(state.current_delete_target().is_none());
        assert!(harness.state().1.is_empty());

        let context = harness.ctx.clone();
        assert!(harness.state_mut().0.copy_selected(&context));
        let mut output = context.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
        assert!(
            output
                .platform_output
                .commands
                .contains(&egui::OutputCommand::CopyText("1\tvalue\n".into()),)
        );
    }
}

#[test]
fn sparse_navigation_moves_displayed_neighbors_without_unfiltered_reads() {
    let mut state = state();
    let mut sparse = page(50, 2, true);
    sparse.row_ordinals = Some(crate::RowOrdinals::new(vec![7, 100]).unwrap());
    state.set_table_data(sparse);
    state
        .table_selection
        .select(state.data.as_ref().unwrap(), 0, 1);
    let target = state.selected_record_target().unwrap();
    assert_eq!(target.parent_ordinal, 7);
    assert!(!state.can_navigate_record(&target));
    assert_eq!(
        state.record_positions(),
        Some(RecordPositions {
            current: 50,
            first: None,
            previous: None,
            next: Some(51),
            last: None,
        })
    );

    let mut actions = Vec::new();
    state.request_display_record(target.clone(), 51, &mut actions);
    assert_eq!(state.table_selection.position(), Some((1, 1)));
    assert_eq!(state.selected_record_target().unwrap().parent_ordinal, 100);
    assert!(actions.is_empty());

    state.request_display_record(target, 52, &mut actions);
    assert_eq!(state.table_selection.position(), Some((1, 1)));
    assert!(state.records.error.is_some());
    assert!(actions.is_empty());
}

#[test]
fn compact_position_editor_uses_original_record_numbers_and_refuses_an_unknown_last() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.0.record_navigator(ui, &mut state.1),
            (state(), Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Last")
            .accesskit_node()
            .is_disabled()
    );
    assert!(harness.query_by_label("of \u{2068}?\u{2069}").is_some());
    let input = harness.get_by_role_and_label(Role::TextInput, "Record number");
    assert_eq!(input.value().as_deref(), Some("101"));
    input.focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Record number")
        .type_text("201");
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::NavigateRecord { target }]
        if target.parent_ordinal == 200 && target.document_revision == 7 && target.column == "ID")
    );
    assert_eq!(harness.state().0.data.as_ref().unwrap().offset, 100);
}

#[test]
fn checked_final_page_count_survives_earlier_pages_without_trusting_out_of_range_eof() {
    let mut state = state();
    assert!(state.checked_record_count().is_none());
    assert!(state.set_table_data(page(100900, 69, false)));
    assert_eq!(state.checked_record_count(), Some(100969));
    assert!(state.set_table_data(page(0, 100, true)));
    assert_eq!(state.checked_record_count(), Some(100969));
    assert_eq!(state.record_positions().unwrap().last, Some(100968));

    state.reset_record_navigation();
    assert!(state.set_table_data(page(200000, 0, false)));
    assert!(state.checked_record_count().is_none());
    assert!(state.set_table_data(page(0, 100, true)));
    assert!(state.record_positions().unwrap().last.is_none());
}

#[test]
fn checked_record_count_is_not_shared_with_another_table_revision_or_mapped_view() {
    let mut state = state();
    assert!(state.set_table_data(page(100, 5, false)));
    let mut another_table = page(0, 100, true);
    another_table.table = "Other".into();
    assert!(state.set_table_data(another_table));
    assert!(state.checked_record_count().is_none());

    assert!(state.set_table_data(page(100, 5, false)));
    let mut another_revision = page(0, 100, true);
    another_revision.source_revision = Some(8);
    state.database = None;
    assert!(state.set_table_data(another_revision));
    assert!(state.checked_record_count().is_none());

    assert!(state.set_table_data(page(100, 5, false)));
    let mut mapped = page(0, 2, false);
    mapped.row_ordinals = Some(crate::RowOrdinals::new(vec![2, 4]).unwrap());
    assert!(state.set_table_data(mapped));
    assert!(state.checked_record_count().is_none());
}
