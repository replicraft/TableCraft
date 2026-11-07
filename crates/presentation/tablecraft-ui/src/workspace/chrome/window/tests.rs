use super::*;
use crate::preview::{Preview, Scene};
use crate::workspace::{Action, State};
use egui::accesskit::Role;
use egui::{ViewportId, vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn inline_record_keeps_window_commands_available_without_file_or_record_dispatch() {
    for (label, command) in [
        ("shell-window-minimize", ViewportCommand::Minimized(true)),
        ("shell-window-maximize", ViewportCommand::Maximized(true)),
        ("shell-window-close", ViewportCommand::Close),
    ] {
        let mut preview = Preview::new(Scene::NewRecord);
        let state = std::mem::take(preview.state_mut());
        assert!(state.inline_insert_active());
        let row_count = state.data.as_ref().unwrap().rows.len();
        let mut harness = Harness::builder()
            .with_size(vec2(1280.0, 820.0))
            .build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| {
                    // macOS keeps its native title bar. Exercise only the same pure
                    // control renderer here; no viewport or OS window is created.
                    if !enabled() {
                        egui::Panel::top("portable_window_control_test").show(ui, |ui| {
                            show(ui, &state.0.localizer);
                        });
                    }
                    state.1.extend(state.0.show(ui));
                },
                (state, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run_steps(2);
        let label = harness.state().0.localizer.text(label);
        let button = harness.get_by_role_and_label(Role::Button, &label);
        assert!(!button.accesskit_node().is_disabled());
        button.click_accesskit();
        harness.run_steps(1);
        assert!(
            harness.output().viewport_output[&ViewportId::ROOT]
                .commands
                .contains(&command)
        );
        assert!(harness.state().1.is_empty());
        assert!(harness.state().0.inline_insert_active());
        assert_eq!(
            harness.state().0.data.as_ref().unwrap().rows.len(),
            row_count
        );
        assert!(
            harness
                .get_by_role_and_label(
                    Role::Button,
                    &harness.state().0.localizer.text("shell-quick-open")
                )
                .accesskit_node()
                .is_disabled()
        );
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[test]
fn inline_record_keeps_the_actual_custom_title_drag_target_enabled() {
    let mut preview = Preview::new(Scene::NewRecord);
    let state = std::mem::take(preview.state_mut());
    let title = state.database.as_ref().unwrap().title.clone();
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    let point = harness.get_by_label(&title).rect().center();
    harness.event(egui::Event::PointerMoved(point));
    harness.event(egui::Event::PointerButton {
        pos: point,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(1);
    harness.event(egui::Event::PointerMoved(point + vec2(30.0, 0.0)));
    harness.run_steps(1);
    assert!(
        harness.output().viewport_output[&ViewportId::ROOT]
            .commands
            .contains(&ViewportCommand::StartDrag)
    );
    assert!(harness.state().1.is_empty());
    assert!(harness.state().0.inline_insert_active());
}
