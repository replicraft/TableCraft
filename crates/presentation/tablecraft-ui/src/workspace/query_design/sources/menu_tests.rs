use super::{source_area, tests::session};
use crate::workspace::query_design::Session;
use egui::{Key, Modifiers, Popup, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::query::{DesignField, DesignFieldMetadata, DesignJoin, DesignJoinKind};

#[test]
fn source_keyboard_menu_creates_only_the_explicit_join_and_escape_restores_its_owner() {
    let mut state = session();
    state.draft.joins.clear();
    state.metadata[1].fields.insert(
        0,
        DesignFieldMetadata {
            name: "Ignored target".into(),
            scalar_type: None,
        },
    );
    let mut harness = Harness::builder()
        .with_size(egui::vec2(700.0, 400.0))
        .build_ui_state(
            |ui, state: &mut Session| {
                source_area(ui, state, 280.0);
            },
            state,
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness.get_all_by_label("*").next().unwrap().focus();
    harness.run_steps(1);
    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    assert!(harness.get_by_label("CustomerID").is_focused());

    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    assert!(Popup::is_any_open(&harness.ctx));
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Join to ⏵")
            .is_focused()
    );
    assert!(harness.state().draft.joins.is_empty());
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(harness.get_by_label("CustomerID").is_focused());

    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Orders ⏵")
            .is_focused()
    );
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert!(
        harness
            .get_all_by_role_and_label(Role::Button, "Ignored target")
            .any(|node| node.is_focused())
    );
    assert!(harness.state().draft.joins.is_empty());
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    assert!(
        harness
            .get_all_by_role_and_label(Role::Button, "OrderCustomer")
            .any(|node| node.is_focused())
    );
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert_eq!(
        harness.state().draft.joins,
        [DesignJoin {
            left: DesignField {
                source_id: 1,
                column: "CustomerID".into(),
            },
            right: DesignField {
                source_id: 2,
                column: "OrderCustomer".into(),
            },
            kind: DesignJoinKind::Inner,
        }]
    );
    assert!(harness.state().draft.columns.is_empty());
    assert!(!Popup::is_any_open(&harness.ctx));
}
