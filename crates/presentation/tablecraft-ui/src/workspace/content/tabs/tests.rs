use super::*;
use crate::{Data, Database, Locale, Localizer, Schema};
use egui::{
    Event, Key, MouseWheelUnit, TouchPhase,
    accesskit::{Action as AccessibilityAction, Role},
};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn harness(width: f32) -> Harness<'static, (State, bool, Vec<Action>)> {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Tab observations".into(),
        format: "ACE 16".into(),
        tables: Vec::new(),
        queries: Vec::new(),
    });
    state.set_table_data(Data {
        source_revision: Some(7),
        table: "Customer accounts with a very long literal name repeated beyond the available tab width".into(),
        offset: 0,
        columns: Vec::new(),
        rows: Vec::new(),
        has_more: false,
    });
    state.set_schema(Schema {
        table: "Customer accounts".into(),
        declared_rows: 0,
        fields: Vec::new(),
    });
    let harness = Harness::builder()
        .with_size(egui::vec2(width, 120.0))
        .build_ui_state(
            |ui, state: &mut (State, bool, Vec<Action>)| {
                crate::localization::install(ui.ctx(), &state.0.localizer);
                ui.add_enabled_ui(state.1, |ui| state.0.document_tabs(ui, &mut state.2));
            },
            (state, true, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness
}

#[test]
fn long_captions_are_single_line_bounded_and_accessibility_retains_full_name() {
    let mut harness = harness(900.0);
    harness.run();
    let name = harness.state().0.data.as_ref().unwrap().table.clone();
    let tab = harness.get_by_role_and_label(Role::Tab, &name);
    assert!(tab.rect().width() <= MAX_TAB_WIDTH);
    assert!(tab.rect().height() <= TAB_HEIGHT);
    let text = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == name => Some(text),
            _ => None,
        })
        .unwrap();
    assert_eq!(text.galley.rows.len(), 1);
    assert!(text.galley.elided);
    assert!(text.pos.x >= tab.rect().left() + TAB_TEXT_PADDING + TAB_ICON_SIZE + TAB_ICON_GAP);
    let colors = palette(&harness.ctx);
    assert!(
        harness.output().shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(frame)
            if frame.rect == tab.rect()
                && frame.fill == colors.title_fill
                && frame.stroke.color == colors.grid_line)
        }),
        "the selected object tab paints a neutral bordered surface"
    );
    assert!(
        harness.output().shapes.iter().any(|shape| {
            let egui::Shape::Rect(image) = &shape.shape else {
                return false;
            };
            if image.brush.is_none() {
                return false;
            }
            let bounds = image.rect;
            tab.rect().contains_rect(bounds)
                && (bounds.width() - TAB_ICON_SIZE).abs() < 1.0
                && (bounds.height() - TAB_ICON_SIZE).abs() < 1.0
                && bounds.right() <= text.pos.x - TAB_ICON_GAP
        }),
        "a small object icon is painted without overlapping the literal caption"
    );
}

#[test]
fn tab_identity_survives_locale_changes_but_rejects_replaced_objects_and_documents() {
    let mut harness = harness(900.0);
    harness.run();
    let query_id = harness
        .get_by_role_and_label(Role::Tab, "Query")
        .accesskit_node()
        .id();
    harness.get_by_role_and_label(Role::Tab, "Query").focus();
    harness.run();
    harness.state_mut().0.localizer = Localizer::new(Locale::De).unwrap();
    harness.run();
    let caption = harness.state().0.localizer.text("shell-query");
    let query = harness.get_by_role_and_label(Role::Tab, &caption);
    assert_eq!(query.accesskit_node().id(), query_id);
    assert!(query.is_focused());
    query.click_accesskit();
    harness.run();
    assert!(harness.state().0.view == DocumentView::Query);

    harness
        .state_mut()
        .0
        .database
        .as_mut()
        .unwrap()
        .document_revision += 1;
    harness.run();
    let query = harness.get_by_role_and_label(Role::Tab, &caption);
    assert_ne!(query.accesskit_node().id(), query_id);
    assert!(!query.is_focused());
    let name = harness.state().0.data.as_ref().unwrap().table.clone();
    let table_id = harness
        .get_by_role_and_label(Role::Tab, &name)
        .accesskit_node()
        .id();
    harness.get_by_role_and_label(Role::Tab, &name).focus();
    harness.run();
    harness.state_mut().0.data.as_mut().unwrap().table = "Replacement table".into();
    harness.run();
    let table = harness.get_by_role_and_label(Role::Tab, "Replacement table");
    assert_ne!(table.accesskit_node().id(), table_id);
    assert!(!table.is_focused());
}

#[test]
fn overflow_tabs_reveal_on_focus_preserve_manual_scroll_and_disable_background_actions() {
    let mut harness = harness(250.0);
    harness.run();
    assert!(harness.get_all_by_role(Role::ScrollBar).next().is_some());
    harness
        .get_by_role_and_label(Role::Tab, "Table Design")
        .focus();
    harness.run();
    let before = harness
        .get_by_role_and_label(Role::Tab, "Table Design")
        .rect();
    assert!(before.right() <= 250.0);
    harness.key_press(Key::Enter);
    harness.run();
    assert!(harness.state().0.view == DocumentView::Design);

    harness.event(Event::PointerMoved(egui::pos2(100.0, before.center().y)));
    harness.event(Event::MouseWheel {
        phase: TouchPhase::Move,
        unit: MouseWheelUnit::Point,
        delta: egui::vec2(500.0, 0.0),
        modifiers: egui::Modifiers::NONE,
    });
    // The hovered literal caption schedules its delayed tooltip while scrolling.
    // Process the real wheel input, then leave the tab before awaiting idle.
    harness.run_steps(3);
    harness.event(Event::PointerGone);
    harness.run();
    let manually_scrolled = harness
        .get_by_role_and_label(Role::Tab, "Table Design")
        .rect();
    assert!(manually_scrolled.left() > before.left());
    harness.run_steps(3);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Tab, "Table Design")
            .rect(),
        manually_scrolled
    );

    harness.state_mut().1 = false;
    harness.run();
    let tab = harness.get_by_role_and_label(Role::Tab, "Query");
    assert!(
        !tab.accesskit_node()
            .data()
            .supports_action(AccessibilityAction::Click)
    );
    assert!(
        !tab.accesskit_node()
            .data()
            .supports_action(AccessibilityAction::Focus)
    );
    tab.click_accesskit();
    harness.run();
    assert!(harness.state().0.view == DocumentView::Design);
}

#[test]
fn close_table_dispatches_exact_source_and_retains_database_and_query_slots() {
    let mut harness = harness(900.0);
    let data = harness.state().0.data.as_ref().unwrap();
    let table = data.table.clone();
    harness.state_mut().0.set_query_data(Data {
        source_revision: None,
        table: "Independent query".into(),
        offset: 0,
        columns: vec!["Value".into()],
        rows: vec![vec![crate::Cell::scalar("retained result")]],
        has_more: false,
    });
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Close table")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().2.as_slice(), [Action::CloseTable { document_revision: 7, table: actual }] if actual == &table)
    );
    let state = &mut harness.state_mut().0;
    assert!(state.close_table(7, &table));
    assert!(state.data.is_none());
    assert!(state.database.is_some());
    assert_eq!(
        state.query_data.as_ref().unwrap().rows[0][0].text,
        "retained result"
    );
    assert!(!state.close_table(7, &table));
}
