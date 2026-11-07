use crate::preview::{Preview, Scene};
use crate::workspace::{Action, State};
use egui::accesskit::Role;
use egui::os::OperatingSystem;
use egui::{Event, Key, Modifiers};
use egui_kittest::{Harness, kittest::Queryable};

fn command(os: OperatingSystem) -> Modifiers {
    Modifiers {
        ctrl: os != OperatingSystem::Mac,
        mac_cmd: os == OperatingSystem::Mac,
        command: true,
        ..Modifiers::NONE
    }
}

fn harness(scene: Scene, os: OperatingSystem) -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(scene);
    let state = std::mem::take(preview.state_mut());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 680.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    harness.ctx.set_os(os);
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
}

fn focus_selected_cell(harness: &mut Harness<'_, (State, Vec<Action>)>) {
    let state = &harness.state().0;
    let (row, column) = state.table_selection.position().unwrap();
    let data = state.data.as_ref().unwrap();
    let mut arguments = crate::FluentArgs::new();
    arguments.set("column", data.columns[column].name.clone());
    arguments.set("row", (u64::from(data.offset) + row as u64 + 1).to_string());
    arguments.set("text", state.selected_cell_text().unwrap().to_owned());
    let value = state.localizer.format("datasheet-cell-context", &arguments);
    harness.get_by_value(&value).focus();
    harness.run_steps(1);
}

fn row_facts(state: &State) -> Vec<Vec<(String, crate::CellKind)>> {
    state
        .data
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| (cell.text.clone(), cell.kind))
                .collect()
        })
        .collect()
}

#[test]
fn native_cell_new_record_chords_dispatch_once_without_application_or_datasheet_zoom() {
    for os in [
        OperatingSystem::Windows,
        OperatingSystem::Nix,
        OperatingSystem::Mac,
    ] {
        for (key, shift) in [(Key::Plus, false), (Key::Plus, true), (Key::Equals, true)] {
            let mut harness = harness(Scene::Datasheet, os);
            crate::theme::set_datasheet_zoom(&harness.ctx, 120);
            harness.run_steps(2);
            focus_selected_cell(&mut harness);
            let target = harness.state().0.current_insert_target().unwrap();
            let original_rows = row_facts(&harness.state().0);
            let zoom = harness.ctx.zoom_factor();
            assert!(harness.ctx.options(|options| options.zoom_with_keyboard));
            let modifiers = Modifiers {
                shift,
                ..command(os)
            };
            // Keep the physical key held: egui derives repeat from keys_down,
            // so a release followed by repeat=true is a fresh press instead.
            harness.event(Event::ModifiersChanged(modifiers));
            harness.event(Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            });
            harness.run_steps(2);
            assert!(
                matches!(harness.state().1.as_slice(), [Action::BeginInsert { target: actual }] if actual == &target),
                "expected one insertion intent for {os:?}, {key:?}, shift={shift}; got {} actions",
                harness.state().1.len()
            );
            assert!(row_facts(&harness.state().0) == original_rows);
            assert_eq!(harness.ctx.zoom_factor(), zoom);
            assert_eq!(crate::theme::datasheet_zoom(&harness.ctx), 120);

            harness.state_mut().1.clear();
            harness.event(Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: true,
                modifiers,
            });
            harness.run_steps(2);
            assert!(
                harness.state().1.is_empty(),
                "held repeat must not dispatch for {os:?}, {key:?}, shift={shift}"
            );
            assert_eq!(harness.ctx.zoom_factor(), zoom);
            assert_eq!(crate::theme::datasheet_zoom(&harness.ctx), 120);
        }
    }
}

#[test]
fn selected_record_header_can_request_a_new_record_without_changing_membership() {
    let mut harness = harness(Scene::Datasheet, OperatingSystem::Windows);
    harness
        .get_all_by_role(Role::RowHeader)
        .next()
        .unwrap()
        .click();
    harness.run_steps(2);
    assert!(harness.state().0.table_selection.row_selected());
    let target = harness.state().0.current_insert_target().unwrap();
    let rows = row_facts(&harness.state().0);
    let zoom = harness.ctx.zoom_factor();
    harness.key_press_modifiers(command(OperatingSystem::Windows), Key::Plus);
    harness.run_steps(2);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::BeginInsert { target: actual }] if actual == &target)
    );
    assert!(row_facts(&harness.state().0) == rows);
    assert_eq!(harness.ctx.zoom_factor(), zoom);
}

#[test]
fn empty_table_new_row_is_a_record_focus_target_without_an_existing_row_handle() {
    let mut harness = harness(Scene::Datasheet, OperatingSystem::Windows);
    let mut data = harness.state_mut().0.data.take().unwrap();
    data.rows.clear();
    data.row_ordinals = None;
    data.has_more = false;
    assert!(harness.state_mut().0.set_table_data(data));
    harness.run_steps(2);
    let header_bottom = harness
        .get_all_by_role(Role::ColumnHeader)
        .map(|node| node.rect().bottom())
        .fold(0.0, f32::max);
    harness
        .get_all_by_role_and_label(Role::Button, "New row")
        .filter(|node| node.rect().top() >= header_bottom)
        .min_by(|left, right| left.rect().top().total_cmp(&right.rect().top()))
        .expect("the appended new-row control is immediately below column headers")
        .focus();
    harness.run_steps(1);
    let zoom = harness.ctx.zoom_factor();
    harness.key_press_modifiers(command(OperatingSystem::Windows), Key::Plus);
    harness.run_steps(2);
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::BeginInsert { .. }]
    ));
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
    assert_eq!(harness.ctx.zoom_factor(), zoom);
}

#[test]
fn text_modal_query_draft_and_object_tab_owners_do_not_start_insertion() {
    for scene in [
        Scene::Query,
        Scene::SavedQuery,
        Scene::InitialTableDraft,
        Scene::Find,
        Scene::Edit,
    ] {
        let mut harness = harness(scene, OperatingSystem::Windows);
        match scene {
            Scene::Query => harness
                .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
                .focus(),
            Scene::Find => harness
                .get_by_role_and_label(Role::TextInput, "Find What:")
                .focus(),
            _ => {}
        }
        harness.run_steps(1);
        harness.key_press_modifiers(command(OperatingSystem::Windows), Key::Plus);
        harness.run_steps(2);
        assert!(
            !harness
                .state()
                .1
                .iter()
                .any(|action| matches!(action, Action::BeginInsert { .. })),
            "scene {scene:?}"
        );
    }

    let mut harness = harness(Scene::Datasheet, OperatingSystem::Windows);
    harness.get_all_by_role(Role::Tab).next().unwrap().focus();
    harness.run_steps(1);
    harness.key_press_modifiers(command(OperatingSystem::Windows), Key::Plus);
    harness.run_steps(2);
    assert!(
        !harness
            .state()
            .1
            .iter()
            .any(|action| matches!(action, Action::BeginInsert { .. }))
    );
}

#[test]
fn bare_equals_and_stale_native_source_never_admit_new_record_intent() {
    let mut harness = harness(Scene::Datasheet, OperatingSystem::Windows);
    focus_selected_cell(&mut harness);
    harness.key_press_modifiers(command(OperatingSystem::Windows), Key::Equals);
    harness.run_steps(2);
    assert!(harness.state().1.is_empty());
    harness
        .state_mut()
        .0
        .database
        .as_mut()
        .unwrap()
        .document_revision += 1;
    harness.key_press_modifiers(command(OperatingSystem::Windows), Key::Plus);
    harness.run_steps(2);
    assert!(harness.state().1.is_empty());
}
