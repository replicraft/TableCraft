use super::*;
use crate::{
    Cell, Data,
    preview::{Preview, Scene},
};
use egui::{Event, FullOutput, PointerButton, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn render(
    state: &mut State,
    context: &egui::Context,
    events: Vec<Event>,
) -> (Vec<Action>, FullOutput) {
    let mut actions = Vec::new();
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100.0, 740.0),
            )),
            time: Some(context.cumulative_frame_nr() as f64 * 0.25),
            events,
            ..Default::default()
        },
        |ui| actions = state.show(ui),
    );
    output.textures_delta.clear();
    (actions, output)
}

fn click(state: &mut State, context: &egui::Context, label: &str) -> (Vec<Action>, FullOutput) {
    context.enable_accesskit();
    render(state, context, Vec::new());
    let (_, output) = render(state, context, Vec::new());
    let button_position = output
        .platform_output
        .accesskit_update
        .as_ref()
        .and_then(|tree| {
            tree.nodes.iter().find_map(|(_, node)| {
                if node.role() == egui::accesskit::Role::Button && node.label() == Some(label) {
                    node.bounds().map(|bounds| {
                        egui::pos2(
                            ((bounds.x0 + bounds.x1) * 0.5) as f32,
                            ((bounds.y0 + bounds.y1) * 0.5) as f32,
                        )
                    })
                } else {
                    None
                }
            })
        });
    let position = button_position
        .or_else(|| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
        })
        .expect("painted command or cell label");
    click_at(state, context, position)
}

fn click_at(
    state: &mut State,
    context: &egui::Context,
    position: egui::Pos2,
) -> (Vec<Action>, FullOutput) {
    render(
        state,
        context,
        vec![
            Event::PointerMoved(position),
            Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    render(
        state,
        context,
        vec![Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    )
}

fn table() -> Data {
    Data {
        row_ordinals: None,
        source_revision: Some(4),
        table: "Contacts".into(),
        offset: 100,
        columns: vec!["ID".into(), "Name".into()],
        rows: vec![vec![
            Cell::scalar("Old first cell"),
            Cell::scalar("Selected full cell text"),
        ]],
        has_more: false,
    }
}

fn native_database() -> crate::Database {
    crate::Database {
        title_path: None,
        document_revision: 4,
        title: "Object views".into(),
        format: "ACE16".into(),
        tables: ["Contacts", "Another table"]
            .into_iter()
            .map(|name| crate::Table {
                local: true,
                name: name.into(),
                system: false,
                storage: "Local".into(),
            })
            .collect(),
        queries: Vec::new(),
    }
}

#[test]
fn home_copy_uses_retained_cell_selection_after_toolbar_focus_changes() {
    let context = egui::Context::default();
    let mut state = State::default();
    state.dismiss_backstage();
    state.set_table_data(table());

    click(&mut state, &context, "Selected full cell text");
    assert_eq!(state.selected_cell_text(), Some("Selected full cell text"));
    let (actions, output) = click(&mut state, &context, "Copy");
    assert!(actions.is_empty());
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText(
                "Selected full cell text".into()
            ))
    );

    // A new page selects its new first cell, without retaining the old non-first
    // coordinate or requiring keyboard focus to copy the replacement value.
    let mut replacement = table();
    replacement.rows[0][0] = Cell::scalar("New first cell");
    state.set_table_data(replacement);
    let (_, output) = click(&mut state, &context, "Copy");
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText("New first cell".into()))
    );
    assert_eq!(state.selected_cell_text(), Some("New first cell"));

    let mut empty = table();
    empty.rows.clear();
    state.set_table_data(empty);
    let (_, output) = click(&mut state, &context, "Copy");
    assert!(
        !output
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(_)))
    );
}

#[test]
fn home_view_menu_uses_existing_table_navigation_and_schema_actions() {
    let context = egui::Context::default();
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(native_database());
    let contacts = crate::ObjectTarget {
        document_revision: 4,
        kind: crate::ObjectKind::Table {
            name: "Contacts".into(),
        },
    };
    state.prepare_object_open(contacts.clone()).unwrap();
    state.set_table_data(table());
    state
        .prepare_object_open(crate::ObjectTarget {
            document_revision: 4,
            kind: crate::ObjectKind::Sql,
        })
        .unwrap();

    let (actions, _) = click(&mut state, &context, "View");
    assert!(actions.is_empty());
    assert!(!egui::Popup::is_any_open(&context));
    assert!(state.view == DocumentView::Query);

    state.prepare_object_open(contacts.clone()).unwrap();
    click(&mut state, &context, "View");
    let (actions, _) = click(&mut state, &context, "Table Design");
    assert!(matches!(actions.as_slice(), [Action::ViewSchema { table }] if table == "Contacts"));
    assert!(!egui::Popup::is_any_open(&context));

    state
        .prepare_object_open(crate::ObjectTarget {
            document_revision: 4,
            kind: crate::ObjectKind::Table {
                name: "Another table".into(),
            },
        })
        .unwrap();
    state.prepare_schema_view();
    state.set_schema(crate::Schema {
        table: "Another table".into(),
        declared_rows: 0,
        fields: Vec::new(),
    });
    assert!(
        state.data.is_none(),
        "opening Design does not scan table records"
    );
    let design = state.active_object().unwrap();
    click(&mut state, &context, "View");
    let (actions, _) = click(&mut state, &context, "Datasheet");
    assert!(matches!(actions.as_slice(), [Action::ViewObjectDatasheet { tab }] if tab == &design));
    assert!(
        state.view == DocumentView::Design,
        "the host owns the actual read transition"
    );
    let open = state.prepare_object_datasheet(&design).unwrap();
    assert!(
        open.needs_load,
        "schema-only tab requests real native records"
    );
    assert_eq!(open.tab, design);

    state.prepare_object_open(contacts).unwrap();
    state.prepare_schema_view();
    state.set_schema(crate::Schema {
        table: "Contacts".into(),
        declared_rows: 1,
        fields: Vec::new(),
    });
    let contacts = state.active_object().unwrap();
    click(&mut state, &context, "View");
    let (actions, _) = click(&mut state, &context, "Datasheet");
    assert!(
        matches!(actions.as_slice(), [Action::ViewObjectDatasheet { tab }] if tab == &contacts)
    );
    let open = state.prepare_object_datasheet(&contacts).unwrap();
    assert!(!open.needs_load, "same-table cached page remains available");
    assert!(state.view == DocumentView::Table);

    // Becoming busy closes an already open menu and refuses its trigger.
    click(&mut state, &context, "View");
    state.busy = true;
    let (actions, _) = click(&mut state, &context, "View");
    assert!(actions.is_empty());
    assert!(!egui::Popup::is_any_open(&context));
    let (actions, _) = click(&mut state, &context, "File");
    assert!(actions.is_empty());
    assert!(state.backstage_visible());
    assert!(state.busy);
}

#[test]
fn row_selector_and_home_copy_share_complete_displayed_record_and_publication_resets_mode() {
    let context = egui::Context::default();
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(super::super::Database {
        title_path: None,
        document_revision: 4,
        title: "Example".into(),
        format: "ACE16".into(),
        tables: vec![],
        queries: vec![],
    });
    state.set_table_data(table());
    let target = state.data.as_ref().unwrap().edit_target(0, 0).unwrap();
    assert!(state.can_edit_cell(&target));
    context.enable_accesskit();
    let (_, output) = render(&mut state, &context, vec![]);
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("actual accessibility tree")
        .nodes
        .iter()
        .find_map(|(_, node)| {
            if node.role() == egui::accesskit::Role::RowHeader
                && node.label() == Some("Select record \u{2068}101\u{2069}")
            {
                node.bounds()
            } else {
                None
            }
        })
        .expect("actual record selector response bounds");
    let position = egui::pos2(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    );
    let (actions, _) = click_at(&mut state, &context, position);
    assert!(actions.is_empty());
    assert!(state.record_selected());
    assert!(state.selected_cell_text().is_none());
    assert!(!state.can_edit_cell(&target));
    let (actions, output) = click(&mut state, &context, "Copy");
    assert!(actions.is_empty());
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText(
                "Old first cell\tSelected full cell text\n".into(),
            ))
    );
    let mut replacement = table();
    replacement.rows[0][0] = "NEW first".into();
    state.set_table_data(replacement);
    assert!(!state.record_selected());
    let (actions, output) = click(&mut state, &context, "Copy");
    assert!(actions.is_empty());
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText("NEW first".into()))
    );
}

fn command_id(output: &FullOutput, label: &str) -> egui::accesskit::NodeId {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("actual command accessibility tree")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == egui::accesskit::Role::Button && node.label() == Some(label))
                .then_some(*id)
        })
        .expect("actual ribbon command")
}

fn assistive_action(target: egui::accesskit::NodeId, action: egui::accesskit::Action) -> Event {
    Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
        action,
        target_tree: egui::accesskit::TreeId::ROOT,
        target_node: target,
        data: None,
    })
}

#[test]
fn working_ribbon_commands_accept_keyboard_and_assistive_activation() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(native_database());
    state.set_table_data(table());
    render(&mut state, &context, vec![]);
    let (_, output) = render(&mut state, &context, vec![]);
    let copy = command_id(&output, "Copy");

    let (actions, _) = render(
        &mut state,
        &context,
        vec![assistive_action(copy, egui::accesskit::Action::Focus)],
    );
    assert!(actions.is_empty());
    assert_eq!(
        context
            .memory(|memory| memory.focused())
            .map(|id| id.accesskit_id()),
        Some(copy)
    );

    for key in [egui::Key::Enter, egui::Key::Space] {
        for pressed in [true, false] {
            let (actions, output) = render(
                &mut state,
                &context,
                vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert!(actions.is_empty());
            let copies: Vec<_> = output
                .platform_output
                .commands
                .iter()
                .filter_map(|command| match command {
                    egui::OutputCommand::CopyText(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            if pressed {
                assert_eq!(copies, ["Old first cell"]);
            } else {
                assert!(copies.is_empty());
            }
        }
    }

    let (actions, output) = render(
        &mut state,
        &context,
        vec![assistive_action(copy, egui::accesskit::Action::Click)],
    );
    assert!(actions.is_empty());
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText("Old first cell".into(),))
    );

    let view = command_id(&output, "View");
    let view_node = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(id, _)| *id == view)
        .map(|(_, node)| node)
        .unwrap();
    assert_eq!(view_node.has_popup(), Some(egui::accesskit::HasPopup::Menu));
    assert_eq!(view_node.is_expanded(), Some(false));
    let (actions, _) = render(
        &mut state,
        &context,
        vec![assistive_action(view, egui::accesskit::Action::Focus)],
    );
    assert!(actions.is_empty());
    let (actions, _) = render(
        &mut state,
        &context,
        vec![Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(actions.is_empty());
    assert!(egui::Popup::is_any_open(&context));
    let (actions, _) = render(
        &mut state,
        &context,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(actions.is_empty());
    assert!(!egui::Popup::is_any_open(&context));

    // Assistive activation can reopen the same menu and choose an existing view.
    render(
        &mut state,
        &context,
        vec![assistive_action(view, egui::accesskit::Action::Click)],
    );
    let (_, output) = render(&mut state, &context, vec![]);
    let design = command_id(&output, "Table Design");
    let (actions, _) = render(
        &mut state,
        &context,
        vec![assistive_action(design, egui::accesskit::Action::Click)],
    );
    assert!(matches!(actions.as_slice(), [Action::ViewSchema { table }] if table == "Contacts"));
}

#[test]
fn disabled_mock_commands_offer_no_assistive_actions_and_reject_forced_activation() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut state = State {
        mock_preview: true,
        ..Default::default()
    };
    state.dismiss_backstage();
    state.set_table_data(table());
    render(&mut state, &context, vec![]);
    let (_, output) = render(&mut state, &context, vec![]);
    let paste = command_id(&output, "Paste");
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    let (_, paste_node) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button && node.label() == Some("Paste"))
        .unwrap();
    assert!(paste_node.is_disabled());
    assert!(!paste_node.supports_action(egui::accesskit::Action::Click));
    assert!(!paste_node.supports_action(egui::accesskit::Action::Focus));

    let (create, create_tab) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Tab && node.label() == Some("Create"))
        .unwrap();
    let create = *create;
    assert!(!create_tab.is_disabled());
    assert!(create_tab.supports_action(egui::accesskit::Action::Click));

    let (actions, output) = render(
        &mut state,
        &context,
        vec![
            assistive_action(paste, egui::accesskit::Action::Focus),
            assistive_action(paste, egui::accesskit::Action::Click),
            Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert!(actions.is_empty());
    assert!(
        !output
            .platform_output
            .commands
            .iter()
            .any(|command| { matches!(command, egui::OutputCommand::CopyText(_)) })
    );
    assert_ne!(
        context
            .memory(|memory| memory.focused())
            .map(|id| id.accesskit_id()),
        Some(paste)
    );
    assert!(state.view == DocumentView::Table);
    assert_eq!(state.selected_cell_text(), Some("Old first cell"));

    // Selecting a mock tab changes only the displayed command panel. Its
    // unavailable commands retain their disabled assistive contract.
    let (actions, _) = render(
        &mut state,
        &context,
        vec![assistive_action(create, egui::accesskit::Action::Click)],
    );
    assert!(actions.is_empty());
    assert!(state.ribbon_tab == ribbon::Tab::Create);
    let (actions, output) = render(&mut state, &context, vec![]);
    assert!(actions.is_empty());
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    let (_, wizard) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button && node.label() == Some("Query Wizard"))
        .unwrap();
    assert!(wizard.is_disabled());
    assert!(!wizard.supports_action(egui::accesskit::Action::Click));
    assert!(!wizard.supports_action(egui::accesskit::Action::Focus));
    assert!(state.view == DocumentView::Table);
    assert_eq!(state.selected_cell_text(), Some("Old first cell"));
}

#[test]
fn mock_table_design_command_prepares_missing_schema_through_the_isolated_preview_host() {
    let preview = Preview::new(Scene::Datasheet);
    assert!(preview.state().schema.is_none());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
    crate::configure(&harness.ctx);
    harness.run();

    harness.get_by_role_and_label(Role::Button, "View").click();
    harness.run();
    let design = harness.get_by_role_and_label(Role::Button, "Table Design");
    assert!(!design.accesskit_node().is_disabled());
    design.click();
    harness.run();

    let state = harness.state().state();
    assert!(state.mock_preview);
    assert!(state.view == DocumentView::Design);
    let schema = state
        .schema
        .as_ref()
        .expect("the preview supplied its synthetic declaration");
    assert_eq!(schema.table, "Customers");
    assert!(!schema.fields.is_empty());
    assert_eq!(state.data.as_ref().unwrap().table, "Customers");
    assert_eq!(state.data.as_ref().unwrap().source_revision, Some(1));

    harness.get_by_role_and_label(Role::Button, "View").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Datasheet")
        .click();
    harness.run();
    assert!(harness.state().state().view == DocumentView::Table);
    assert!(harness.state().state().mock_preview);

    harness.state_mut().state_mut().busy = true;
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "View")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn native_read_commands_emit_source_bound_full_table_sort_and_current_page_refresh() {
    let mut preview = Preview::new(Scene::Datasheet);
    let mut state = std::mem::take(preview.state_mut());
    state.mock_preview = false;
    let native = state.data.as_ref().unwrap();
    let revision = native.source_revision.unwrap();
    let table = native.table.clone();
    let column = native.columns[0].name.clone();
    let offset = native.offset;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Ascending")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::SortView { target, sort: Some(sort), enabled: false }] if target.document_revision == revision && target.table == table && target.filter_id.is_none() && sort.column == column && !sort.descending)
    );
    assert!(
        harness
            .state()
            .0
            .data
            .as_ref()
            .unwrap()
            .source_revision
            .is_some()
    );
    harness.state_mut().1.clear();
    harness
        .get_by_role_and_label(Role::Button, "Refresh All")
        .click_accesskit();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Refresh")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::Page { table: actual, offset: actual_offset }] if actual == &table && *actual_offset == offset)
    );
    harness.state_mut().1.clear();
    let mut arguments = crate::FluentArgs::new();
    arguments.set("column", column.as_str());
    let header_label = harness
        .state()
        .0
        .localizer
        .format("datasheet-column-menu", &arguments);
    harness
        .get_by_role_and_label(Role::Button, &header_label)
        .click_accesskit();
    harness.run();
    harness
        .get_all_by_role_and_label(Role::Button, "Descending")
        .last()
        .expect("opened header sort command")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::SortView { target, sort: Some(sort), enabled: false }] if target.document_revision == revision && target.table == table && target.filter_id.is_none() && sort.column == column && sort.descending)
    );
    assert!(!harness.state().0.can_sort_view(
        &crate::SortTarget {
            document_revision: revision + 1,
            table: table.clone(),
            filter_id: None
        },
        Some(&crate::SortSpec {
            column: column.clone(),
            descending: false
        }),
        false
    ));
    assert!(!harness.state().0.can_sort_view(
        &crate::SortTarget {
            document_revision: revision,
            table: table.clone(),
            filter_id: None
        },
        Some(&crate::SortSpec {
            column: "missing column".into(),
            descending: false
        }),
        false
    ));
}

#[test]
fn home_paste_requests_clipboard_then_emits_literal_review_intent() {
    let context = egui::Context::default();
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(native_database());
    state.set_table_data(table());
    let (actions, output) = click(&mut state, &context, "Paste");
    assert!(actions.is_empty());
    let requested = output.viewport_output.values().any(|viewport| {
        viewport
            .commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::RequestPaste))
    });
    assert!(requested, "Home requests real clipboard text");

    for _ in 0..2 {
        let (actions, _) = render(&mut state, &context, Vec::new());
        assert!(actions.is_empty());
        assert!(
            state.paste_request.is_some(),
            "clipboard reply may be delayed"
        );
    }

    let (actions, _) = render(
        &mut state,
        &context,
        vec![Event::Paste("NULL\nnext\tfield".into())],
    );
    assert!(matches!(
        actions.as_slice(),
        [Action::PasteCell { target, text }]
            if target.column == "ID" && text == "NULL\nnext\tfield"
    ));
    assert!(!state.edit_dialog_open(), "host must request a fresh draft");
}

#[test]
fn focused_native_cell_accepts_paste_but_edit_dialog_keeps_local_clipboard_input() {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(native_database());
    state.set_table_data(table());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 740.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    let target = harness.state().0.current_paste_target().unwrap();
    let context = harness.ctx.clone();
    harness
        .state_mut()
        .0
        .table_selection
        .focus_selected(&context);
    harness.run();
    harness.event(Event::Paste("literal native text".into()));
    harness.run();
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::PasteCell { target: actual, text }]
            if actual == &target && text == "literal native text"
    ));

    let context = harness.ctx.clone();
    harness.state_mut().0.request_paste(&context);
    let state = &mut harness.state_mut().0;
    assert!(state.begin_paste_edit(target.clone(), "".into(), 1));
    state.set_edit_draft(
        1,
        crate::EditDraft {
            id: 2,
            target,
            original: Cell::scalar("original"),
            initial: crate::EditInput::default(),
            input_hint: crate::InputHint::Text,
            nullable: true,
        },
    );
    harness.state_mut().1.clear();
    harness.run();
    harness.get_by_role(Role::MultilineTextInput).focus();
    harness.run();
    harness.event(Event::Paste("stale Home clipboard".into()));
    harness.run();
    assert!(harness.state().1.is_empty());
    let Some(super::super::editing::EditDialog::Draft { input, .. }) = &harness.state().0.edit
    else {
        panic!("draft remains open after stale clipboard response");
    };
    assert!(input.text.is_empty());
    assert!(harness.state().0.error.is_some());

    harness.event(Event::Paste("dialog text".into()));
    harness.run();
    assert!(harness.state().1.is_empty());
    let Some(super::super::editing::EditDialog::Draft { input, .. }) = &harness.state().0.edit
    else {
        panic!("draft remains open");
    };
    assert_eq!(input.text, "dialog text");
}

#[test]
fn mock_ribbon_tabs_reveal_disabled_command_panels_without_changing_the_open_object() {
    let preview = Preview::new(Scene::Datasheet);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    let object = harness.state().state().active_object().unwrap();
    let rows = harness.state().state().data.as_ref().unwrap().rows.as_ptr();

    for (tab, command) in [
        ("Create", "Query Wizard"),
        ("External Data", "New Data Source"),
        ("Database Tools", "Compact and Repair"),
        ("Help", "Help"),
        ("Table Fields", "New Field"),
        ("Table", "Before Change"),
    ] {
        harness
            .get_by_role_and_label(Role::Tab, tab)
            .click_accesskit();
        harness.run_steps(3);
        assert_eq!(
            harness
                .get_by_role_and_label(Role::Tab, tab)
                .accesskit_node()
                .is_selected(),
            Some(true)
        );
        let control = harness.get_by_role_and_label(Role::Button, command);
        assert!(control.accesskit_node().is_disabled());
        assert_eq!(
            harness.state().state().active_object(),
            Some(object.clone())
        );
        assert_eq!(
            harness.state().state().data.as_ref().unwrap().rows.as_ptr(),
            rows
        );
        assert!(!harness.state().state().busy);
    }

    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Copy")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn normal_home_keeps_all_native_groups_and_unavailable_controls_without_granting_actions() {
    let mut preview = Preview::new(Scene::Datasheet);
    let mut state = std::mem::take(preview.state_mut());
    state.mock_preview = false;
    state.replace_available = false;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 900.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    for key in [
        "ribbon-views",
        "ribbon-clipboard",
        "ribbon-sort-filter",
        "ribbon-records",
        "ribbon-find-group",
        "ribbon-text-formatting",
        "ribbon-zoom",
    ] {
        let label = harness.state().0.localizer.text(key);
        assert!(
            harness
                .query_by_role_and_label(Role::Label, &label)
                .is_some(),
            "missing normal Home group: {label}"
        );
    }
    for key in ["ribbon-paste", "ribbon-new", "ribbon-delete"] {
        let label = harness.state().0.localizer.text(key);
        harness.get_by_role_and_label(Role::Button, &label);
    }
    for key in [
        "ribbon-cut",
        "ribbon-format-painter",
        "ribbon-save",
        "ribbon-spelling",
        "ribbon-more",
        "ribbon-bold",
        "ribbon-italic",
        "ribbon-underline",
        "ribbon-font-color",
        "ribbon-text-highlight",
        "ribbon-fill-color",
        "ribbon-align-left",
        "ribbon-align-center",
        "ribbon-align-right",
        "ribbon-borders",
        "ribbon-alternate-rows",
        "ribbon-bullets",
        "ribbon-numbering",
        "ribbon-decrease-indent",
        "ribbon-increase-indent",
        "ribbon-text-direction",
    ] {
        let label = harness.state().0.localizer.text(key);
        // Quick Access also has Save; the Home command follows it in the tree.
        let node = harness
            .get_all_by_role_and_label(Role::Button, &label)
            .last()
            .expect("visible Home command");
        assert!(
            node.accesskit_node().is_disabled(),
            "unsupported command enabled: {label}"
        );
        assert!(
            !node
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Click)
        );
        assert!(
            !node
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Focus)
        );
        node.focus();
        node.click_accesskit();
    }
    for key in ["ribbon-font", "ribbon-font-size"] {
        let label = harness.state().0.localizer.text(key);
        let node = harness.get_by_role_and_label(Role::ComboBox, &label);
        assert!(node.accesskit_node().is_disabled());
        assert!(
            !node
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Click)
        );
        node.focus();
        node.click_accesskit();
    }
    for group in ["ribbon-clipboard", "ribbon-text-formatting"] {
        let label = format!(
            "{}: {}",
            harness.state().0.localizer.text(group),
            harness.state().0.localizer.text("ribbon-group-options")
        );
        let node = harness.get_by_role_and_label(Role::Button, &label);
        assert!(node.accesskit_node().is_disabled());
        node.focus();
        node.click_accesskit();
    }
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Replace")
            .accesskit_node()
            .is_disabled()
    );
    let original = harness.state().0.selected_cell_text().map(str::to_owned);
    harness.run();
    assert!(harness.state().1.is_empty());
    assert_eq!(
        harness.state().0.selected_cell_text().map(str::to_owned),
        original
    );
    harness
        .get_by_role_and_label(Role::Button, "Copy")
        .click_accesskit();
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText(original.unwrap()))
    );
}
