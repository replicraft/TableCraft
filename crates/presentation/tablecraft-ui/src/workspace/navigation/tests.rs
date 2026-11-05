use super::*;
use crate::{Database, SavedQuery, Table};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn navigation_rows_use_full_width_and_retain_exact_object_identity_after_filtering() {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        title_path: None,
        document_revision: 17,
        title: "Synthetic navigation".into(),
        format: "ACE 16".into(),
        tables: ["First", "Second"]
            .into_iter()
            .map(|name| Table {
                name: name.into(),
                system: false,
                storage: "Local".into(),
            })
            .collect(),
        queries: vec![SavedQuery {
            object_id: 91,
            name: "Second".into(),
            system: false,
        }],
    });
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    assert!(harness.query_by_label("All Objects").is_some());
    assert!(harness.query_by_label("All Access Objects").is_none());
    let search = harness.get_by_role_and_label(Role::TextInput, "Search objects");
    assert!(search.rect().height() >= SEARCH_HEIGHT);
    assert!(
        harness.output().shapes.iter().any(|shape| {
            let egui::Shape::Rect(image) = &shape.shape else {
                return false;
            };
            image.brush.is_some()
                && search.rect().contains_rect(image.rect)
                && (image.rect.width() - SEARCH_ICON_SIZE).abs() < 1.0
        }),
        "the taller search field contains its magnifier artwork"
    );

    let rows: Vec<_> = harness
        .get_all_by_role_and_label(Role::Button, "Second")
        .collect();
    assert_eq!(rows.len(), 2, "table and query retain distinct commands");
    assert_ne!(rows[0].accesskit_node().id(), rows[1].accesskit_node().id());
    let table_id = rows[0].accesskit_node().id();
    let bounds = rows[0].rect();
    assert!(
        bounds.width() > 170.0,
        "the whole navigation row is interactive"
    );
    rows[0].focus();
    harness.run();
    assert!(
        harness
            .get_all_by_role_and_label(Role::Button, "Second")
            .next()
            .unwrap()
            .is_focused()
    );

    harness.state_mut().0.filter = "Second".into();
    harness.run();
    let rows: Vec<_> = harness
        .get_all_by_role_and_label(Role::Button, "Second")
        .collect();
    assert_eq!(rows[0].accesskit_node().id(), table_id);
    assert!(
        rows[0].is_focused(),
        "filtering earlier rows must not retarget focus"
    );
    let bounds = rows[0].rect();
    let blank_point = egui::pos2(bounds.right() - 8.0, bounds.center().y);
    harness.event(egui::Event::PointerMoved(blank_point));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: blank_point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run();
    assert!(harness.state().1.is_empty(), "single click only selects");
    assert_eq!(
        harness.state().0.selected_navigation_object(),
        Some(ObjectTarget {
            document_revision: 17,
            kind: ObjectKind::Table {
                name: "Second".into()
            },
        }),
    );
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::OpenObject { target }]
            if target.document_revision == 17
                && target.kind == ObjectKind::Table { name: "Second".into() }
    ));

    harness.state_mut().1.clear();
    harness
        .get_all_by_role_and_label(Role::Button, "Second")
        .nth(1)
        .unwrap()
        .click_accesskit();
    harness.run();
    assert!(
        harness.state().1.is_empty(),
        "query selection never inspects or runs"
    );
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::OpenObject { target }]
            if target.document_revision == 17
                && target.kind == ObjectKind::SavedQuery {
                    object_id: 91,
                    name: "Second".into(),
                }
    ));
    harness.state_mut().1.clear();
    let query_bounds = harness
        .get_all_by_role_and_label(Role::Button, "Second")
        .nth(1)
        .unwrap()
        .rect();
    let point = query_bounds.center();
    harness.event(egui::Event::PointerMoved(point));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Open")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::OpenObject { target }]
        if matches!(target.kind, ObjectKind::SavedQuery { object_id: 91, .. }))
    );
    harness.state_mut().1.clear();

    let selected = harness.state().0.selected_navigation_object();
    harness.key_press(egui::Key::F11);
    harness.run();
    assert!(!harness.state().0.navigation_visible());
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Search objects")
            .is_none()
    );
    assert!(harness.state().1.is_empty());
    harness.key_press(egui::Key::F11);
    harness.run();
    assert!(harness.state().0.navigation_visible());
    assert_eq!(harness.state().0.selected_navigation_object(), selected);
    assert!(harness.state().1.is_empty());

    harness
        .state_mut()
        .0
        .database
        .as_mut()
        .unwrap()
        .document_revision = 18;
    harness.run();
    let rows: Vec<_> = harness
        .get_all_by_role_and_label(Role::Button, "Second")
        .collect();
    assert_ne!(rows[0].accesskit_node().id(), table_id);
    assert!(
        rows.iter().all(|row| !row.is_focused()),
        "a replaced document must not inherit an old object focus"
    );
    assert!(harness.state().1.is_empty());
}
