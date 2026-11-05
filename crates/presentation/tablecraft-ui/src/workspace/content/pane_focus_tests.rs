use super::*;
use crate::{
    Cell,
    preview::{Preview, Scene},
};
use egui::accesskit::Role;
use egui::{Event, Key, MouseWheelUnit, TouchPhase};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn harness(scene: Scene) -> Harness<'static, State> {
    let mut preview = Preview::new(scene);
    let state = std::mem::take(preview.state_mut());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .with_max_steps(20)
        .build_ui_state(
            |ui, state: &mut State| assert!(state.show(ui).is_empty()),
            state,
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness
}

fn scroll_away(harness: &mut Harness<'_, State>, position: egui::Pos2) {
    harness.event(Event::PointerMoved(position));
    harness.event(Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -10000.0),
        modifiers: egui::Modifiers::NONE,
        phase: TouchPhase::Move,
    });
    harness.run_steps(3);
    harness.event(Event::PointerGone);
    harness.run();
}

#[test]
fn pane_entry_reveals_offscreen_selected_cell_or_record_without_changing_selection() {
    for query in [false, true] {
        for record in [false, true] {
            let mut harness = harness(if query {
                Scene::Query
            } else {
                Scene::Datasheet
            });
            let state = harness.state_mut();
            let data = if query {
                state.query_data.as_mut().unwrap()
            } else {
                state.data.as_mut().unwrap()
            };
            data.rows = (0..200)
                .map(|row| {
                    data.columns
                        .iter()
                        .map(|_| Cell::scalar(format!("row {row}")))
                        .collect()
                })
                .collect();
            harness.run();
            let first = harness.get_all_by_role(Role::RowHeader).next().unwrap();
            let first_label = first.accesskit_node().label().unwrap();
            let position = first.rect().center() + egui::vec2(100.0, 0.0);
            if record {
                first.click();
                harness.run();
            }
            scroll_away(&mut harness, position);
            assert!(
                harness.query_by_label(&first_label).is_none(),
                "selected first row was virtualized away"
            );

            harness.get_by_role_and_label(Role::Tab, "Home").focus();
            harness.run();
            harness.key_press(Key::F6);
            harness.run();
            if query {
                harness.key_press(Key::F6);
                harness.run();
            }
            let state = harness.state();
            let selection = if query {
                &state.query_selection
            } else {
                &state.table_selection
            };
            assert_eq!(selection.position(), Some((0, 0)));
            assert_eq!(selection.row_selected(), record);
            if record {
                let row = harness.get_by_label(&first_label);
                assert!(row.is_focused());
            } else {
                assert!(
                    harness
                        .get_all_by_role(Role::Label)
                        .any(|node| node.is_focused())
                );
            }
            assert!(
                harness.query_by_label(&first_label).is_some(),
                "pane entry reveals original selected row"
            );
        }
    }
}

#[test]
fn compact_design_pane_entry_reveals_selected_field_and_focuses_readonly_property() {
    let mut harness = harness(Scene::Design);
    let schema = harness.state_mut().schema.as_mut().unwrap();
    schema.fields = (0..200)
        .map(|number| crate::Field {
            name: format!("Focus field {number}"),
            number,
            native_type: 4,
            storage_bytes: 4,
            fixed: true,
            flags: 0,
            extended_flags: 0,
            scalar_type: Some("Int32".into()),
            display_type: None,
            field_size: None,
            reader_nullable: None,
            decimal_parameters: None,
            complex_type_id: None,
            properties: vec![crate::FieldProperty {
                name: "Caption",
                value: crate::FieldPropertyValue::Text("Literal readonly property".into()),
            }],
        })
        .collect();
    harness.run();
    let position = harness.get_by_label("Focus field 0").rect().center();
    scroll_away(&mut harness, position);
    assert!(harness.query_by_label("Focus field 0").is_none());
    assert_eq!(harness.state().selected_field, 0);

    harness.get_by_role_and_label(Role::Tab, "Home").focus();
    harness.run();
    harness.key_press(Key::F6);
    harness.run();
    let field = harness.get_by_label("Focus field 0");
    assert!(
        field.is_focused(),
        "selected field focus missing, raw focus {:?}, entry {}",
        harness.ctx.memory(|memory| memory.focused()),
        harness.state().pane_focus_entry.is_some()
    );
    assert_eq!(harness.state().selected_field, 0);

    harness.key_press(Key::F6);
    harness.run();
    let property = harness.get_by_label("Literal readonly property");
    assert!(
        property.is_focused(),
        "Properties pane uses a real readonly value response"
    );
    assert!(property.rect().top() >= 0.0 && property.rect().bottom() <= 580.0);
    assert_eq!(harness.state().selected_field, 0);
}

#[test]
fn newer_navigation_focus_after_read_failure_supersedes_deferred_pane_entry() {
    let mut harness = harness(Scene::Datasheet);
    harness.get_by_role_and_label(Role::Tab, "Home").focus();
    harness.run();
    harness.event(Event::Key {
        key: Key::F6,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    assert!(harness.state().pane_focus_entry.is_some());

    harness.state_mut().error = Some("Read failed; previous page retained.".into());
    harness
        .get_by_role_and_label(Role::TextInput, "Search objects")
        .focus();
    harness.run();
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Search objects")
            .is_focused()
    );
    assert!(harness.state().pane_focus_entry.is_none());
    assert_eq!(harness.state().table_selection.position(), Some((0, 0)));
}
