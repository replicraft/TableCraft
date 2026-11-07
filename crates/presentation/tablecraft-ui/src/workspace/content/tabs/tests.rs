use super::*;
use crate::{Data, Database, Locale, Localizer, Table};
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
    let name =
        "Customer accounts with a very long literal name repeated beyond the available tab width";
    state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Tab observations".into(),
        format: "ACE 16".into(),
        tables: vec![Table {
            local: true,
            name: name.into(),
            system: false,
            storage: "Local".into(),
        }],
        queries: Vec::new(),
    });
    state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Table { name: name.into() },
        })
        .unwrap();
    state.set_table_data(Data {
        row_ordinals: None,
        source_revision: Some(7),
        table: name.into(),
        offset: 0,
        columns: Vec::new(),
        rows: Vec::new(),
        has_more: false,
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
fn actual_object_tabs_have_stable_instance_identity_and_no_view_placeholders() {
    let mut harness = harness(900.0);
    harness.run();
    assert_eq!(harness.get_all_by_role(Role::Tab).count(), 1);
    assert!(
        harness
            .query_by_role_and_label(Role::Tab, "Query")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Tab, "Table Design")
            .is_none()
    );

    let table = harness.state().0.active_object().unwrap();
    let name = match &table.target.kind {
        ObjectKind::Table { name } => name.clone(),
        _ => panic!("actual table object"),
    };
    let table_id = harness
        .get_by_role_and_label(Role::Tab, &name)
        .accesskit_node()
        .id();
    harness.state_mut().0.set_schema(crate::Schema {
        table: name.clone(),
        declared_rows: 0,
        fields: Vec::new(),
    });
    harness.run();
    assert_eq!(harness.get_all_by_role(Role::Tab).count(), 1);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Tab, &name)
            .accesskit_node()
            .id(),
        table_id
    );
    assert_eq!(harness.state().0.active_object(), Some(table));

    let sql = ObjectTarget {
        document_revision: 7,
        kind: ObjectKind::Sql,
    };
    harness
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    harness.run();
    assert!(matches!(
        harness.state().2.as_slice(),
        [Action::OpenObject { target }] if target == &sql
    ));
    harness.state_mut().2.clear();
    harness
        .state_mut()
        .0
        .prepare_object_open(sql.clone())
        .unwrap();
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
    let old = harness.state().0.active_object().unwrap();
    assert!(harness.state_mut().0.close_object(&old));
    harness.state_mut().0.prepare_object_open(sql).unwrap();
    harness.run();
    let query = harness.get_by_role_and_label(Role::Tab, &caption);
    assert_ne!(query.accesskit_node().id(), query_id);
    assert!(
        !query.is_focused(),
        "reopened object is a fresh tab instance"
    );
    assert!(!harness.state_mut().0.activate_object(&old));
}

#[test]
fn overflow_tabs_reveal_on_focus_preserve_manual_scroll_and_disable_background_actions() {
    let mut harness = harness(250.0);
    harness
        .state_mut()
        .0
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap();
    harness.run();
    assert!(harness.get_all_by_role(Role::ScrollBar).next().is_some());
    harness.get_by_role_and_label(Role::Tab, "Query").focus();
    harness.run();
    let before = harness.get_by_role_and_label(Role::Tab, "Query").rect();
    assert!(before.right() <= 250.0);
    harness.key_press(Key::Enter);
    harness.run();
    assert!(
        matches!(harness.state().2.as_slice(), [Action::ActivateObject { tab }]
        if tab.target.kind == ObjectKind::Sql)
    );
    harness.state_mut().2.clear();

    harness.event(Event::PointerMoved(egui::pos2(100.0, before.center().y)));
    harness.event(Event::MouseWheel {
        phase: TouchPhase::Move,
        unit: MouseWheelUnit::Point,
        delta: egui::vec2(500.0, 0.0),
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(3);
    harness.event(Event::PointerGone);
    harness.run();
    let manually_scrolled = harness.get_by_role_and_label(Role::Tab, "Query").rect();
    assert!(manually_scrolled.left() > before.left());
    harness.run_steps(3);
    assert_eq!(
        harness.get_by_role_and_label(Role::Tab, "Query").rect(),
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
    assert!(harness.state().2.is_empty());
}

#[test]
fn close_dispatches_exact_tab_instance_and_retains_independent_sql_object() {
    let mut harness = harness(900.0);
    let table = harness.state().0.active_object().unwrap();
    harness
        .state_mut()
        .0
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap();
    harness.state_mut().0.set_query_data(
        None,
        Data {
            row_ordinals: None,
            source_revision: None,
            table: "Independent query".into(),
            offset: 0,
            columns: vec!["Value".into()],
            rows: vec![vec![crate::Cell::scalar("retained result")]],
            has_more: false,
        },
    );
    harness.run();
    harness
        .get_all_by_role_and_label(Role::Button, "Close object")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run();
    assert!(matches!(harness.state().2.as_slice(), [Action::CloseObject { tab }] if tab == &table));
    let state = &mut harness.state_mut().0;
    assert!(state.close_object(&table));
    assert!(state.database.is_some());
    assert_eq!(state.object_tabs().len(), 1);
    assert_eq!(
        state.query_data.as_ref().unwrap().rows[0][0].text,
        "retained result"
    );
    assert!(!state.close_object(&table));
}
