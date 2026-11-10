use crate::workspace::{
    Action, State,
    preview::{Preview, Scene},
};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::table::DraftFieldType;

fn harness() -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(Scene::InitialTableDraft);
    let mut state = std::mem::take(preview.state_mut());
    state.table_save_available = true;
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| {
                state.1.extend(state.0.show(ui));
            },
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
}

#[test]
fn new_header_selects_temporary_name_and_enter_or_tab_commits_exact_text() {
    for key in [egui::Key::Enter, egui::Key::Tab] {
        let mut harness = harness();
        let input = harness.get_by_role_and_label(Role::TextInput, "Field name");
        assert!(input.is_focused());
        let input_rect = input.rect();
        assert!(input_rect.height() <= 22.0);
        harness
            .input_mut()
            .events
            .push(egui::Event::Text("Customer".into()));
        harness.run_steps(1);
        harness.key_press(key);
        harness.run_steps(3);
        let editor = harness.state().0.table_draft.as_ref().unwrap();
        assert_eq!(editor.draft.fields()[1].name(), "Customer");
        assert!(editor.rename.is_none());
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Click to Add")
                .is_focused()
        );
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn escape_discards_only_pending_name_and_retains_declared_field() {
    let mut harness = harness();
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("Not committed".into()));
    harness.run_steps(1);
    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);
    let state = &harness.state().0;
    let editor = state.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields().len(), 2);
    assert_eq!(editor.draft.fields()[1].name(), "Field1");
    assert!(editor.rename.is_none());
    assert!(state.has_modified_table_drafts());
}

#[test]
fn duplicate_name_blocks_save_without_losing_input_then_correction_saves_exact_name() {
    let mut harness = harness();
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("ID".into()));
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert!(!harness.state_mut().0.request_table_draft_save());
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.rename.as_ref().unwrap().name, "ID");
    assert_eq!(editor.draft.fields()[1].name(), "Field1");
    assert!(editor.error);
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field name")
            .is_focused()
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run_steps(1);
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("Name".into()));
    harness.run_steps(1);
    assert!(harness.state_mut().0.request_table_draft_save());
    assert_eq!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()[1]
            .name(),
        "Name"
    );
    assert!(harness.state().0.table_draft_save_open());
    assert!(harness.state().1.is_empty());
}

#[test]
fn pending_name_survives_parking_and_protected_id_never_becomes_an_editor() {
    let mut harness = harness();
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("Pending".into()));
    harness.run_steps(1);
    let tab = harness.state().0.active_object().unwrap();
    let scratch = harness
        .state_mut()
        .0
        .prepare_object_open(crate::ObjectTarget {
            document_revision: tab.target.document_revision,
            kind: crate::ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert!(harness.state_mut().0.activate_object(&tab));
    assert_eq!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .rename
            .as_ref()
            .unwrap()
            .name,
        "Pending"
    );
    assert!(harness.state().0.has_modified_table_drafts());
    assert!(harness.state().0.object_tabs().contains(&scratch));
    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);
    let header = harness.get_by_role_and_label(Role::ColumnHeader, "ID");
    header.click();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::ColumnHeader, "ID")
        .click();
    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Field name")
            .is_none()
    );
}

#[test]
fn same_frame_text_and_save_shortcut_commit_the_current_header_before_naming() {
    let mut harness = harness();
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("Latest name".into()));
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.run_steps(2);
    let state = &harness.state().0;
    assert_eq!(
        state.table_draft.as_ref().unwrap().draft.fields()[1].name(),
        "Latest name"
    );
    assert!(state.table_draft_save_open());
    assert!(harness.state().1.is_empty());
}

#[test]
fn full_draft_keeps_last_name_and_manual_scroll_does_not_refocus_disabled_placeholder() {
    let mut harness = harness();
    // The initial helper adds Field1. Add the remaining companion declarations
    // through the same shared command without manufacturing editor state.
    for _ in 2..255 {
        assert!(
            harness
                .state_mut()
                .0
                .add_table_draft_field(DraftFieldType::Number)
        );
    }
    harness.run_steps(2);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(3);
    let before = harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .draft
        .snapshot("Table1")
        .unwrap();
    assert_eq!(before.fields().len(), 255);
    assert_eq!(before.fields()[254].name(), "Field254");
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Click to Add")
            .is_focused()
    );
    assert_horizontal_filler(&harness);
    harness.event(egui::Event::PointerMoved(egui::pos2(700.0, 400.0)));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(100_000.0, 0.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(10);
    let first = harness
        .get_by_role_and_label(Role::ColumnHeader, "Field1")
        .rect();
    assert!(first.intersects(harness.ctx.content_rect()));
    harness.run_steps(4);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::ColumnHeader, "Field1")
            .rect(),
        first
    );
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert!(editor.draft.snapshot("Table1").unwrap() == before);
    assert!(editor.rename.is_none());
    assert_horizontal_filler(&harness);
    assert!(harness.state().1.is_empty());
}

#[test]
fn manual_horizontal_scroll_releases_trailing_add_focus_and_keeps_first_field_visible() {
    let mut harness = harness();
    for _ in 2..20 {
        assert!(
            harness
                .state_mut()
                .0
                .add_table_draft_field(DraftFieldType::Number)
        );
    }
    harness.run_steps(2);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Click to Add")
            .is_focused()
    );
    let ordinary_header = harness
        .get_by_role_and_label(Role::ColumnHeader, "Field19")
        .rect()
        .intersect(harness.ctx.content_rect());
    assert!(ordinary_header.is_positive());
    let pointer = ordinary_header.center();
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Click to Add")
            .rect()
            .contains(pointer)
    );
    assert_horizontal_filler(&harness);
    harness.event(egui::Event::PointerMoved(pointer));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(100_000.0, 0.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(10);
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Click to Add")
            .is_focused()
    );
    assert!(
        harness
            .get_by_role_and_label(Role::ColumnHeader, "Field1")
            .rect()
            .intersects(harness.ctx.content_rect())
    );
    harness.run_steps(4);
    assert!(
        harness
            .get_by_role_and_label(Role::ColumnHeader, "Field1")
            .rect()
            .intersects(harness.ctx.content_rect())
    );
    assert_horizontal_filler(&harness);
    assert!(harness.state().1.is_empty());
}

fn assert_horizontal_filler(harness: &Harness<'_, (State, Vec<Action>)>) {
    let color = crate::theme::palette(&harness.ctx)
        .grid_line
        .gamma_multiply(0.4);
    let rows = harness
        .output()
        .shapes
        .iter()
        .find_map(|clipped| {
            let egui::Shape::Vec(lines) = &clipped.shape else {
                return None;
            };
            let horizontal = lines
                .iter()
                .filter_map(|shape| match shape {
                    egui::Shape::LineSegment { points, stroke }
                        if stroke.color == color && points[0].y == points[1].y =>
                    {
                        Some(points)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            (horizontal.len() > 3).then_some(horizontal)
        })
        .expect("empty draft space retains horizontal filler at this scroll end");
    for points in rows {
        assert!(points[1].x - points[0].x > 500.0);
        assert!(harness.ctx.content_rect().contains(points[0]));
        assert!(harness.ctx.content_rect().contains(points[1]));
    }
}
