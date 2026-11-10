//! Source-free maximum-count layout driven by the shared draft commands.
use super::LAYOUT_SETTLE_STEPS;
use egui::{Event, Key, Modifiers, MouseWheelUnit, TouchPhase, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::table::DraftFieldType;
use tablecraft_ui::preview::Preview;

pub(super) fn show(harness: &mut Harness<'_, Preview>, last: bool) {
    for _ in 1..255 {
        assert!(
            harness
                .state_mut()
                .state_mut()
                .add_table_draft_field(DraftFieldType::Number)
        );
    }
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness.key_press(Key::Enter);
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    // The trailing add control keeps itself visible while focused. Move focus
    // without dispatching a command before exercising actual horizontal input.
    let home = harness.state().state().localizer.text("ribbon-home");
    harness.get_by_role_and_label(Role::Tab, &home).focus();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness.event(Event::PointerMoved(egui::pos2(700.0, 400.0)));
    harness.event(Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: egui::vec2(if last { -100_000.0 } else { 100_000.0 }, 0.0),
        phase: TouchPhase::Move,
        modifiers: Modifiers::NONE,
    });
    harness.run_steps(10);
    let field = if last { "Field254" } else { "Field1" };
    let header = harness.get_by_role_and_label(Role::ColumnHeader, field);
    assert!(header.rect().intersects(harness.ctx.content_rect()));
    let status = harness
        .state()
        .state()
        .localizer
        .text("table-draft-field-limit");
    assert!(harness.query_by_label(&status).is_some());
}
