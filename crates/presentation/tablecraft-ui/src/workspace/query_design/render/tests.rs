use crate::preview::{Preview, Scene};
use crate::workspace::{Action, State};
use egui::{Event, Key, Modifiers, PointerButton, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::{cell::RefCell, rc::Rc};

#[test]
fn blank_qbe_cells_and_keyboard_splitter_preserve_the_draft_and_center_show_boxes() {
    let mut preview = Preview::new(Scene::QueryDesign);
    let state = std::mem::take(preview.state_mut());
    let actions = Rc::new(RefCell::new(Vec::<Action>::new()));
    let observed_actions = Rc::clone(&actions);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 820.0))
        .build_ui_state(
            move |ui, state: &mut State| observed_actions.borrow_mut().extend(state.show(ui)),
            state,
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    let original_sql = harness.state().sql.clone();
    let original_draft = harness.state().query_design_draft().unwrap().clone();
    let original_request = harness.state().query_design_request().unwrap();
    let inputs = harness
        .get_all_by_role_and_label(Role::TextInput, "Field expression")
        .collect::<Vec<_>>();
    assert_eq!(inputs.len(), 2);
    let first = inputs[0].rect();
    let second = inputs[1].rect();
    let column_width = second.left() - first.left();
    let show = harness
        .get_by_role_and_label(Role::CheckBox, "Show: 1")
        .rect();
    let glyph = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Rect(rectangle)
                if rectangle.rect.width() >= 8.0
                    && rectangle.rect.width() <= 24.0
                    && (rectangle.rect.width() - rectangle.rect.height()).abs() < 1.0
                    && (rectangle.rect.center().y - show.center().y).abs() < 1.0
                    && rectangle.rect.center().x >= first.left()
                    && rectangle.rect.center().x <= first.right() =>
            {
                Some(rectangle.rect)
            }
            _ => None,
        })
        .expect("the actual Show checkbox glyph");
    assert!(
        (glyph.center().x - first.center().x).abs() <= 1.0,
        "glyph {glyph:?}, first {first:?}, Show {show:?}"
    );

    let blank = egui::pos2(second.center().x + column_width, first.center().y);
    harness.event(Event::PointerMoved(blank));
    for pressed in [true, false] {
        harness.event(Event::PointerButton {
            pos: blank,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run_steps(2);
    assert_eq!(harness.state().query_design_draft(), Some(&original_draft));
    assert_eq!(
        harness
            .get_all_by_role_and_label(Role::TextInput, "Field expression")
            .count(),
        2
    );

    harness
        .get_by_role_and_label(Role::Button, "Resize source panes and query grid")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::ArrowDown);
    harness.run_steps(3);
    let moved_top = harness
        .get_all_by_role_and_label(Role::TextInput, "Field expression")
        .next()
        .unwrap()
        .rect()
        .top();
    assert!(
        moved_top > first.top() + 10.0,
        "the actual grid follows the keyboard-adjusted divider"
    );
    assert_eq!(harness.state().sql, original_sql);
    assert_eq!(harness.state().query_design_draft(), Some(&original_draft));
    assert_eq!(
        harness.state().query_design_request(),
        Some(original_request)
    );
    assert!(harness.state().query_data.is_none());
    assert!(
        actions.borrow().is_empty(),
        "layout gestures must not run or build queries"
    );
}
