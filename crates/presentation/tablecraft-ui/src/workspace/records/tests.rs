use super::*;
use crate::Database;

fn page(offset: u32, count: usize, has_more: bool) -> Data {
    Data {
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
    assert!(!state.set_record_data(9, target.clone(), page(200, 3, false)));
    assert!(!state.set_record_data(10, target.clone(), page(199, 4, false)));
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
    assert!(state.set_record_data(11, target, page(200, 3, false)));
    assert_eq!(state.table_selection.position(), Some((0, 1)));
    assert_eq!(state.data.as_ref().unwrap().offset, 200);
}

#[test]
fn checked_eof_failure_and_replaced_view_preserve_the_previous_page() {
    let mut state = state();
    let mut target = state.selected_record_target().unwrap();
    target.parent_ordinal = 999;
    state.begin_record_navigation(target.clone(), 10);
    assert!(!state.set_record_data(10, target.clone(), page(999, 0, false)));
    assert_eq!(state.data.as_ref().unwrap().offset, 100);
    assert!(state.records.error.as_ref().unwrap().contains("no record"));

    state.begin_record_navigation(target.clone(), 11);
    state.fail_record_navigation(11, "Read cancelled".into());
    assert_eq!(state.data.as_ref().unwrap().offset, 100);
    assert_eq!(state.records.error.as_deref(), Some("Read cancelled"));

    state.begin_record_navigation(target.clone(), 12);
    state.select_view(DocumentView::Query);
    assert!(!state.set_record_data(12, target, page(999, 1, false)));
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
    let mut state = state();
    let context = egui::Context::default();
    let mut actions = Vec::new();
    let mut output = context.run_ui(input(Vec::new()), |ui| {
        state.record_navigator(ui, &mut actions)
    });
    output.textures_delta.clear();
    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == "Previous" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("actual Previous button label");
    for pressed in [true, false] {
        let mut output = context.run_ui(
            input(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]),
            |ui| state.record_navigator(ui, &mut actions),
        );
        output.textures_delta.clear();
    }
    assert!(
        matches!(actions.as_slice(), [Action::NavigateRecord { target }] if target.parent_ordinal == 99 && target.document_revision == 7 && target.table == "Items")
    );
    assert_eq!(state.data.as_ref().unwrap().offset, 100);
    assert_eq!(state.table_selection.position(), Some((0, 0)));
}

fn input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1100.0, 740.0),
        )),
        events,
        ..Default::default()
    }
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
    let mut state = state();
    state.sql = "SELECT 1".into();
    let context = egui::Context::default();
    let render = |state: &mut State, events| {
        let mut actions = Vec::new();
        let mut output = context.run_ui(input(events), |ui| actions = state.show(ui));
        output.textures_delta.clear();
        actions
    };
    assert!(render(&mut state, Vec::new()).is_empty());
    let record_input = egui::Id::new("record_number");
    context.memory_mut(|memory| memory.request_focus(record_input));
    assert!(render(&mut state, vec![egui::Event::Text("301".into())]).is_empty());
    assert_eq!(state.records.input, "301");
    assert_eq!(
        context.memory(|memory| memory.focused()),
        Some(record_input)
    );

    let actions = render(&mut state, vec![record_enter(true, egui::Modifiers::NONE)]);
    assert!(
        matches!(actions.as_slice(), [Action::NavigateRecord { target }]
        if target.document_revision == 7 && target.table == "Items"
            && target.parent_ordinal == 300 && target.column == "ID")
    );
    assert!(render(&mut state, vec![record_enter(false, egui::Modifiers::NONE)]).is_empty());
    assert!(render(&mut state, Vec::new()).is_empty());
    assert_eq!(state.data.as_ref().unwrap().offset, 100);

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
        context.memory_mut(|memory| memory.request_focus(record_input));
        assert!(render(&mut state, Vec::new()).is_empty());
        assert_eq!(
            context.memory(|memory| memory.focused()),
            Some(record_input)
        );
        assert!(render(&mut state, vec![record_enter(true, modifiers)]).is_empty());
        assert!(render(&mut state, vec![record_enter(false, modifiers)]).is_empty());
        assert!(render(&mut state, Vec::new()).is_empty());
    }
}

fn record_enter(pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}
