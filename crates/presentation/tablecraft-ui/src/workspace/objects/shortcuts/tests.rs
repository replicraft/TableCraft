use super::super::tests::{state, table};
use super::*;
use crate::preview::{Preview, Scene};
use crate::{ObjectKind, ObjectTarget};
use egui::accesskit::Role;
use egui::os::OperatingSystem;
use egui::{Event, Key, Modifiers, ViewportCommand, ViewportId};
use egui_kittest::{Harness, kittest::Queryable};

fn command(os: OperatingSystem) -> Modifiers {
    Modifiers {
        ctrl: os != OperatingSystem::Mac,
        mac_cmd: os == OperatingSystem::Mac,
        command: true,
        ..Modifiers::NONE
    }
}

fn harness(state: State, os: OperatingSystem) -> Harness<'static, (State, Vec<Action>)> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 680.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    harness.ctx.set_os(os);
    crate::configure(&harness.ctx);
    harness.run();
    harness
}

#[test]
fn object_chords_emit_exact_instances_without_deleting_sql_or_replacing_pane_shortcuts() {
    for os in [
        OperatingSystem::Windows,
        OperatingSystem::Nix,
        OperatingSystem::Mac,
    ] {
        let mut state = state();
        let first = state.prepare_object_open(table("A")).unwrap().tab;
        let second = state.prepare_object_open(table("B")).unwrap().tab;
        let scratch = state
            .prepare_object_open(ObjectTarget {
                document_revision: 7,
                kind: ObjectKind::Sql,
            })
            .unwrap()
            .tab;
        state.sql = "SELECT one two".into();
        let mut harness = harness(state, os);
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
            .focus();
        harness.run_steps(1);
        harness.key_press(Key::End);
        harness.run_steps(1);
        harness.key_press_modifiers(command(os), Key::W);
        harness.run_steps(1);
        assert!(
            matches!(harness.state().1.as_slice(), [Action::CloseObject { tab }] if tab == &scratch)
        );
        assert_eq!(harness.state().0.sql, "SELECT one two");
        assert_eq!(harness.state().0.active_object(), Some(scratch.clone()));
        harness.state_mut().1.clear();

        harness.key_press_modifiers(command(os), Key::F6);
        harness.run_steps(1);
        assert!(
            matches!(harness.state().1.as_slice(), [Action::ActivateObject { tab }] if tab == &first)
        );
        harness.state_mut().1.clear();
        harness.key_press_modifiers(
            Modifiers {
                shift: true,
                ..command(os)
            },
            Key::F6,
        );
        harness.run_steps(1);
        assert!(
            matches!(harness.state().1.as_slice(), [Action::ActivateObject { tab }] if tab == &second)
        );
        harness.state_mut().1.clear();

        for (key, modifiers) in [
            (Key::F6, Modifiers::NONE),
            (Key::F6, Modifiers::SHIFT),
            (Key::Tab, command(os)),
        ] {
            harness.key_press_modifiers(modifiers, key);
            harness.run_steps(1);
            assert!(harness.state().1.is_empty());
        }
        assert_eq!(harness.state().0.active_object(), Some(scratch));

        if os == OperatingSystem::Mac {
            harness
                .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
                .focus();
            harness.run_steps(1);
            harness.key_press(Key::End);
            harness.run_steps(1);
            let previous = harness.state().0.sql.clone();
            harness.key_press_modifiers(Modifiers::CTRL, Key::W);
            harness.run_steps(1);
            assert!(harness.state().1.is_empty());
            assert_ne!(harness.state().0.sql, previous);
        } else {
            harness.key_press_modifiers(command(os), Key::F4);
            harness.run_steps(1);
            assert!(matches!(
                harness.state().1.as_slice(),
                [Action::CloseObject { .. }]
            ));
        }
    }
}

#[test]
fn busy_modal_and_save_owners_consume_close_without_window_or_object_fallback() {
    for scene in [
        Scene::QueryRunning,
        Scene::CollectionLoading,
        Scene::SavedParameters,
        Scene::SaveApplying,
    ] {
        let mut preview = Preview::new(scene);
        let mut state = std::mem::take(preview.state_mut());
        let context = egui::Context::default();
        context.set_os(OperatingSystem::Windows);
        crate::configure(&context);
        let mut warm = context.run_ui(egui::RawInput::default(), |ui| {
            state.show(ui);
        });
        warm.textures_delta.clear();
        let active = state
            .active_object()
            .expect("the blocked scene owns a real object");
        let mut actions = Vec::new();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 680.0),
                )),
                events: vec![Event::Key {
                    key: Key::W,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: command(OperatingSystem::Windows),
                }],
                ..Default::default()
            },
            |ui| {
                actions = state.show(ui);
            },
        );
        assert!(actions.is_empty());
        assert_eq!(state.active_object(), Some(active));
        assert!(
            output.viewport_output[&ViewportId::ROOT]
                .commands
                .contains(&ViewportCommand::CancelClose)
        );
        output.textures_delta.clear();
    }
}
