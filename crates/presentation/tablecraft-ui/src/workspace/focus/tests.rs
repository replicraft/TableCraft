use super::*;
use egui::Event;

#[test]
fn current_frame_cycles_only_enabled_focusable_anchors_without_dispatching_actions() {
    let context = Context::default();
    let mut state = State::default();
    state.dismiss_backstage();
    let mut document = Id::NULL;
    let mut ribbon = Id::NULL;
    let mut results = Id::NULL;
    let mut stale = Id::NULL;
    for frame in 0..4 {
        let events = if frame == 1 || frame == 2 {
            vec![Event::Key {
                key: Key::F6,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: if frame == 2 {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                },
            }]
        } else {
            Vec::new()
        };
        let mut output = context.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                state.begin_pane_focus(ui, false);
                let response = ui.button("Document anchor");
                document = response.id;
                register(ui, Pane::Document, &response, true);
                let response = ui.button("Results anchor");
                results = response.id;
                register(ui, Pane::Results, &response, true);
                let response = ui.button("Ribbon anchor");
                ribbon = response.id;
                register(ui, Pane::Ribbon, &response, true);
                let response = ui.add_enabled(false, egui::Button::new("Disabled navigation"));
                register(ui, Pane::Navigation, &response, true);
                if frame == 0 {
                    let response = ui.button("Removed pane");
                    stale = response.id;
                    register(ui, Pane::Properties, &response, true);
                    context.memory_mut(|memory| memory.request_focus(document));
                }
                state.finish_pane_focus(ui);
            },
        );
        output.textures_delta.clear();
        let focused = context.memory(|memory| memory.focused());
        if frame == 1 {
            assert_eq!(focused, Some(results));
        }
        if frame == 2 {
            assert_eq!(focused, Some(document));
        }
        assert_ne!(focused, Some(stale));
        if frame > 0 {
            assert_ne!(focused, Some(ribbon));
        }
    }
}

#[test]
fn pane_entry_rechecks_source_publication_and_exact_modifiers_are_not_hijacked() {
    let context = Context::default();
    let mut state = State::default();
    state.dismiss_backstage();
    let publication = |text: &str| crate::Data {
        source_revision: Some(7),
        table: "Same table".into(),
        offset: 0,
        columns: vec!["Value".into()],
        rows: vec![vec![crate::Cell::scalar(text)]],
        has_more: false,
    };
    state.set_table_data(publication("Original"));
    let scope = state.pane_scope();
    state.pane_focus_entry = Some(Entry {
        scope,
        pane: Pane::Document,
        focus_witness: None,
    });

    // A fresh publication may retain the exact same source/name/page metadata.
    state.set_table_data(publication("Replacement"));
    assert!(state.take_pane_entry(&context).is_none());
    assert_eq!(state.selected_cell_text(), Some("Replacement"));
    assert_ne!(scope, state.pane_scope());

    state.pane_focus_entry = Some(Entry {
        scope,
        pane: Pane::Document,
        focus_witness: None,
    });
    assert!(state.take_pane_entry(&context).is_none());

    for modifiers in [
        Modifiers::CTRL,
        Modifiers::ALT,
        Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        },
    ] {
        let mut output = context.run_ui(
            egui::RawInput {
                events: vec![Event::Key {
                    key: Key::F6,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ui| {
                state.begin_pane_focus(ui, false);
                assert!(ui.input(|input| {
                    input
                        .events
                        .iter()
                        .any(|event| matches!(event, Event::Key { key: Key::F6, .. }))
                }));
                let button = ui.button("Current anchor");
                register(ui, Pane::Document, &button, true);
                state.finish_pane_focus(ui);
                assert!(state.take_pane_entry(&context).is_none());
            },
        );
        output.textures_delta.clear();
    }
}

#[test]
fn modal_backstage_and_popups_suppress_pane_navigation_before_and_after_render() {
    enum Gate {
        Modal,
        Backstage,
        PopupBefore,
        PopupDuring,
    }

    for gate in [
        Gate::Modal,
        Gate::Backstage,
        Gate::PopupBefore,
        Gate::PopupDuring,
    ] {
        let context = Context::default();
        let mut state = State::default();
        if !matches!(gate, Gate::Backstage) {
            state.dismiss_backstage();
        }
        let mut output = context.run_ui(
            egui::RawInput {
                events: vec![Event::Key {
                    key: Key::F6,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                let current = ui.button("Current document");
                context.memory_mut(|memory| memory.request_focus(current.id));
                if matches!(gate, Gate::PopupBefore) {
                    Popup::open_id(&context, Id::new("test_popup"));
                }
                state.begin_pane_focus(ui, matches!(gate, Gate::Modal));
                register(ui, Pane::Document, &current, true);
                let target = ui.button("Ribbon target");
                register(ui, Pane::Ribbon, &target, true);
                if matches!(gate, Gate::PopupDuring) {
                    Popup::open_id(&context, Id::new("test_popup"));
                }
                state.finish_pane_focus(ui);
                assert_eq!(context.memory(|memory| memory.focused()), Some(current.id));
                assert!(state.take_pane_entry(&context).is_none());
            },
        );
        output.textures_delta.clear();
    }
}

#[test]
fn disabled_host_and_subtree_cannot_declare_or_enter_loaded_panes() {
    let context = Context::default();
    let mut state = State::default();
    state.dismiss_backstage();
    state.set_table_data(crate::Data {
        source_revision: Some(7),
        table: "Loaded data".into(),
        offset: 0,
        columns: vec!["Value".into()],
        rows: vec![vec![crate::Cell::scalar("Keep selection")]],
        has_more: false,
    });
    let mut output = context.run_ui(
        egui::RawInput {
            events: vec![Event::Key {
                key: Key::F6,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ui| {
            ui.add_enabled_ui(false, |ui| {
                assert!(state.show(ui).is_empty());
                assert!(state.take_pane_entry(&context).is_none());
                assert!(ui.input(|input| input.events.iter().any(|event| {
                    matches!(
                        event,
                        Event::Key {
                            key: Key::F6,
                            pressed: true,
                            ..
                        }
                    )
                })));
            });
            assert_eq!(state.selected_cell_text(), Some("Keep selection"));

            // An active workspace must also reject anchorless declarations from
            // a nested disabled surface, even when its source has records.
            state.begin_pane_focus(ui, false);
            ui.add_enabled_ui(false, |ui| declare(ui, Pane::Document));
            let frame = context
                .data(|data| data.get_temp::<Option<Frame>>(registry_id()).flatten())
                .expect("the enabled workspace frame is active");
            assert!(frame.panes.is_empty());
            state.finish_pane_focus(ui);
            assert!(state.take_pane_entry(&context).is_none());
        },
    );
    output.textures_delta.clear();
}
