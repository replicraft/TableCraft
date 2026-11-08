mod description;
mod new_field;

use crate::workspace::{
    Action, EditInput, InsertInput, State,
    preview::{Preview, Scene},
};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::table::{DraftFieldId, DraftFieldType};

fn state() -> (State, DraftFieldId) {
    let mut preview = Preview::new(Scene::InitialTableDraft);
    let mut state = std::mem::take(preview.state_mut());
    state.table_save_available = true;
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    assert!(state.table_draft.as_mut().unwrap().commit_rename());
    let field = state.table_draft.as_ref().unwrap().draft.fields()[1].id();
    (state, field)
}

fn harness(state: State) -> Harness<'static, (State, Vec<Action>)> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

fn field_label(state: &State, key: &str, field: &str) -> String {
    let mut arguments = crate::FluentArgs::new();
    arguments.set("field", field);
    state.localizer.format(key, &arguments)
}

#[test]
fn ribbon_status_and_tab_view_switches_share_one_editor_without_native_actions() {
    let (state, _) = state();
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Table Design")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().0.table_draft_design_visible());
    assert!(harness.state().0.schema.is_none());
    assert!(!harness.state().0.active_object_load().unwrap().needs_load);
    harness
        .get_by_role_and_label(Role::Button, "Datasheet view")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().0.table_draft_design_visible());
    harness
        .get_by_role_and_label(Role::Tab, "Table1")
        .click_secondary();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Table Design")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().0.table_draft_design_visible());
    assert!(harness.state().1.is_empty());
    assert!(
        harness
            .state()
            .0
            .database
            .as_ref()
            .unwrap()
            .tables
            .is_empty()
    );
}

#[test]
fn design_name_and_type_editors_retain_identity_and_raw_record_input() {
    let (mut state, field) = state();
    assert!(state.table_draft.as_mut().unwrap().record.set_input(
        field,
        InsertInput::Value(EditInput {
            text: "0042".into(),
            is_null: false
        })
    ));
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(
            Role::Button,
            &field_label(&harness.state().0, "table-draft-design-name", "Field1"),
        )
        .click_accesskit();
    harness.run_steps(2);
    harness.event(egui::Event::Text("Score".into()));
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(
            Role::ComboBox,
            &field_label(&harness.state().0, "table-draft-design-type", "Score"),
        )
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Number")
        .click_accesskit();
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields()[1].id(), field);
    assert_eq!(editor.draft.fields()[1].kind(), DraftFieldType::Number);
    let snapshot = editor.draft.snapshot("Items").unwrap();
    assert_eq!(editor.record.raw_texts(&snapshot).unwrap()[1], "0042");
    assert!(harness.state_mut().0.show_table_draft_datasheet());
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(
                Role::TextInput,
                &field_label(&harness.state().0, "table-draft-record-field", "Score")
            )
            .value()
            .as_deref(),
        Some("0042")
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn parked_design_view_keeps_name_buffer_and_invalid_name_blocks_switch_and_save() {
    let (mut state, field) = state();
    assert!(state.show_table_draft_design());
    let draft = state.active_object().unwrap();
    let scratch = state
        .prepare_object_open(crate::ObjectTarget {
            document_revision: draft.target.document_revision,
            kind: crate::ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert!(state.activate_object(&draft));
    assert!(state.table_draft_design_visible());
    assert!(state.object_tabs().contains(&scratch));
    state.table_draft.as_mut().unwrap().rename =
        Some(super::rename::Rename::new(field, "ID".into()));
    assert!(!state.show_table_draft_datasheet());
    assert!(!state.request_table_draft_save());
    assert!(state.table_draft_design_visible());
    assert_eq!(
        state
            .table_draft
            .as_ref()
            .unwrap()
            .rename
            .as_ref()
            .unwrap()
            .name,
        "ID"
    );
}

#[test]
fn type_changes_preserve_hidden_omitted_null_and_invalid_boolean_text_without_conversion() {
    for mode in 0..3 {
        let (mut state, field) = state();
        assert!(state.table_draft.as_mut().unwrap().record.set_input(
            field,
            InsertInput::Value(EditInput {
                text: "not a boolean".into(),
                is_null: mode == 2
            })
        ));
        if mode == 0 {
            assert!(
                state
                    .table_draft
                    .as_mut()
                    .unwrap()
                    .record
                    .set_input(field, InsertInput::Omitted)
            );
        }
        assert!(state.set_table_draft_field_type(field, DraftFieldType::YesNo));
        let editor = state.table_draft.as_ref().unwrap();
        let snapshot = editor.draft.snapshot("Items").unwrap();
        assert_eq!(
            editor.record.raw_texts(&snapshot).unwrap()[1],
            "not a boolean"
        );
        match &editor.record.inputs(&snapshot).unwrap()[1] {
            InsertInput::Omitted => assert_eq!(mode, 0),
            InsertInput::Value(value) => assert_eq!(value.is_null, mode == 2),
        }
        if mode == 1 {
            let mut harness = harness(state);
            let input_label = field_label(&harness.state().0, "table-draft-record-field", "Field1");
            let input = harness.get_by_role_and_label(Role::TextInput, &input_label);
            assert_eq!(input.value().as_deref(), Some("not a boolean"));
            assert!(
                harness
                    .query_by_role_and_label(
                        Role::CheckBox,
                        &field_label(&harness.state().0, "table-draft-record-field", "Field1")
                    )
                    .is_none()
            );
            harness
                .get_by_role_and_label(
                    Role::TextInput,
                    &field_label(&harness.state().0, "table-draft-record-field", "Field1"),
                )
                .focus();
            harness.run_steps(1);
            harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
            harness.event(egui::Event::Text("true".into()));
            harness.run_steps(2);
            assert!(
                harness
                    .query_by_role_and_label(
                        Role::CheckBox,
                        &field_label(&harness.state().0, "table-draft-record-field", "Field1")
                    )
                    .is_some()
            );
            assert!(harness.state().1.is_empty());
        }
    }
}

#[test]
fn protected_id_and_pending_confirmation_or_save_refuse_type_and_view_changes() {
    let (mut state, field) = state();
    let primary = state.table_draft.as_ref().unwrap().draft.fields()[0].id();
    assert!(!state.set_table_draft_field_type(primary, DraftFieldType::ShortText));
    assert!(!state.set_table_draft_field_type(field, DraftFieldType::AutoNumberIncrement));
    assert!(state.request_table_draft_field_delete(field));
    assert!(!state.show_table_draft_design());
    assert!(!state.set_table_draft_field_type(field, DraftFieldType::Number));
    assert!(state.cancel_table_draft_field_delete());
    assert!(state.request_table_draft_save());
    assert!(!state.show_table_draft_design());
    assert!(!state.set_table_draft_field_type(field, DraftFieldType::Number));
}

#[test]
fn compact_design_keeps_save_and_both_panes_visible_across_languages_and_themes() {
    for locale in crate::Locale::ALL {
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            let (mut state, _) = state();
            state.localizer = crate::Localizer::new(locale).unwrap();
            state.theme = theme;
            assert!(state.show_table_draft_design());
            let save = state.localizer.text("table-draft-save");
            let properties = state.localizer.text("design-field-properties");
            let field_name = state.localizer.text("design-field-name");
            let mut compact = Harness::builder()
                .with_size(egui::vec2(900.0, 580.0))
                .build_ui_state(
                    |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                    (state, Vec::new()),
                );
            crate::configure(&compact.ctx);
            compact.run_steps(3);
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 580.0));
            assert!(
                compact
                    .get_all_by_role_and_label(Role::Button, &save)
                    .any(|node| viewport.contains_rect(node.rect()))
            );
            assert!(
                viewport.contains_rect(compact.get_by_label(&properties).rect()),
                "{locale:?} {theme:?}"
            );
            assert!(
                compact
                    .get_all_by_label(&field_name)
                    .any(|node| viewport.contains_rect(node.rect()))
            );
            assert!(compact.state().1.is_empty());
            assert!(
                compact
                    .state()
                    .0
                    .database
                    .as_ref()
                    .unwrap()
                    .tables
                    .is_empty()
            );
        }
    }
}

#[test]
fn refused_type_request_cannot_commit_a_pending_name_or_change_input_revision() {
    let (mut state, field) = state();
    let primary = state.table_draft.as_ref().unwrap().draft.fields()[0].id();
    state.table_draft.as_mut().unwrap().rename =
        Some(super::rename::Rename::new(field, "Pending".into()));
    let schema_revision = state.table_draft_revision();
    let record_revision = state.table_draft.as_ref().unwrap().record_revision();
    for target in [primary, field] {
        assert!(!state.set_table_draft_field_type(target, DraftFieldType::AutoNumberIncrement));
    }
    assert_eq!(state.table_draft_revision(), schema_revision);
    assert_eq!(
        state.table_draft.as_ref().unwrap().record_revision(),
        record_revision
    );
    assert_eq!(
        state.table_draft.as_ref().unwrap().draft.fields()[1].name(),
        "Field1"
    );
    assert_eq!(
        state
            .table_draft
            .as_ref()
            .unwrap()
            .rename
            .as_ref()
            .unwrap()
            .name,
        "Pending"
    );
}

#[test]
fn another_name_cell_cannot_replace_an_invalid_pending_name_or_its_record_input() {
    let (mut state, first) = state();
    assert!(state.add_table_draft_field(DraftFieldType::Number));
    assert!(state.table_draft.as_mut().unwrap().commit_rename());
    assert!(state.table_draft.as_mut().unwrap().record.set_input(
        first,
        InsertInput::Value(EditInput {
            text: "Ada".into(),
            is_null: false,
        })
    ));
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(
            Role::Button,
            &field_label(&harness.state().0, "table-draft-design-name", "Field1"),
        )
        .click_accesskit();
    harness.run_steps(2);
    harness.event(egui::Event::Text("ID".into()));
    harness.run_steps(1);
    harness
        .get_by_role_and_label(
            Role::Button,
            &field_label(&harness.state().0, "table-draft-design-name", "Field2"),
        )
        .click();
    harness.run_steps(3);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    let pending = editor.rename.as_ref().unwrap();
    assert_eq!(pending.field, first);
    assert_eq!(pending.name, "ID");
    assert_eq!(editor.draft.fields()[1].name(), "Field1");
    assert_eq!(editor.draft.fields()[2].name(), "Field2");
    let snapshot = editor.draft.snapshot("Items").unwrap();
    assert_eq!(editor.record.raw_texts(&snapshot).unwrap()[1], "Ada");
    assert!(!harness.state_mut().0.request_table_draft_save());
    assert!(harness.state().1.is_empty());
}

#[test]
fn blocked_parked_draft_view_requests_leave_the_active_sql_session_untouched() {
    let (mut state, field) = state();
    let draft = state.active_object().unwrap();
    let editor = state.table_draft.as_mut().unwrap();
    assert!(editor.record.set_input(
        field,
        InsertInput::Value(EditInput {
            text: "0007 retained input".into(),
            is_null: false,
        }),
    ));
    editor.save_blocked = true;
    let snapshot = editor.draft.snapshot("Items").unwrap();
    let record_revision = editor.record_revision();
    let scratch = state
        .prepare_object_open(crate::ObjectTarget {
            document_revision: draft.target.document_revision,
            kind: crate::ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    state.sql = "SELECT 7 AS Retained".into();

    assert!(state.can_activate_object(&draft));
    assert!(!state.can_view_object_design(&draft));
    assert!(!state.can_view_object_datasheet(&draft));
    assert!(state.prepare_object_design(&draft).is_none());
    assert!(state.prepare_object_datasheet(&draft).is_none());

    assert_eq!(state.active_object(), Some(scratch));
    assert_eq!(state.sql, "SELECT 7 AS Retained");
    let editor = state.table_draft_editor(&draft).unwrap();
    assert!(editor.draft.snapshot("Items").unwrap() == snapshot);
    assert_eq!(editor.record_revision(), record_revision);
    assert_eq!(
        editor.record.raw_texts(&snapshot).unwrap()[1],
        "0007 retained input"
    );
    assert!(editor.save_blocked);
}

#[test]
fn view_switch_consumes_current_name_text_before_committing_either_surface() {
    for design in [false, true] {
        let (mut state, _) = state();
        if design {
            assert!(state.show_table_draft_design());
        }
        let mut harness = harness(state);
        if design {
            harness
                .get_by_role_and_label(
                    Role::Button,
                    &field_label(&harness.state().0, "table-draft-design-name", "Field1"),
                )
                .click_accesskit();
            harness.run_steps(2);
        } else {
            harness
                .get_by_role_and_label(Role::ColumnHeader, "Field1")
                .click_secondary();
            harness.run_steps(2);
            harness
                .get_by_role_and_label(Role::Button, "Rename Field")
                .click_accesskit();
            harness.run_steps(2);
        }
        assert!(
            harness
                .get_by_role_and_label(Role::TextInput, "Field name")
                .is_focused()
        );
        harness.event(egui::Event::Text("Current frame name".into()));
        harness
            .get_by_role_and_label(
                Role::Button,
                if design {
                    "Datasheet view"
                } else {
                    "Design view"
                },
            )
            .click_accesskit();
        harness.run_steps(3);
        assert_eq!(harness.state().0.table_draft_design_visible(), !design);
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
            "Current frame name"
        );
        assert!(harness.state().1.is_empty());
    }
}

pub(super) fn paint_leaves<'a>(shape: &'a egui::Shape, output: &mut Vec<&'a egui::Shape>) {
    match shape {
        egui::Shape::Vec(children) => {
            for child in children {
                paint_leaves(child, output);
            }
        }
        leaf => output.push(leaf),
    }
}

#[test]
fn upper_grid_paint_is_clipped_before_properties_and_blank_space_is_inert_ruled_decoration() {
    for size in [egui::vec2(1280.0, 820.0), egui::vec2(900.0, 580.0)] {
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            let (mut state, _) = state();
            state.theme = theme;
            assert!(state.show_table_draft_design());
            let mut harness = Harness::builder().with_size(size).build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                (state, Vec::new()),
            );
            crate::configure(&harness.ctx);
            harness.run_steps(3);
            let header = harness
                .get_all_by_label("Field Name")
                .map(|node| node.rect())
                .min_by(|left, right| left.top().total_cmp(&right.top()))
                .unwrap();
            let properties_top = harness.get_by_label("Field Properties").rect().top();
            let addition = harness
                .get_by_role_and_label(Role::TextInput, "New field name")
                .rect();
            let colors = crate::theme::palette(&harness.ctx);
            let mut column_rules = 0;
            let mut blank_rules = 0;
            let mut selector_backgrounds = 0;
            for clipped in &harness.output().shapes {
                let mut leaves = Vec::new();
                paint_leaves(&clipped.shape, &mut leaves);
                for shape in leaves {
                    match shape {
                        egui::Shape::LineSegment { points, stroke } => {
                            let vertical = (points[0].x - points[1].x).abs() < 0.1;
                            let top = points[0].y.min(points[1].y);
                            let bottom = points[0].y.max(points[1].y);
                            if vertical
                                && points[0].x >= header.left()
                                && top <= header.bottom()
                                && bottom >= addition.bottom()
                            {
                                column_rules += 1;
                                assert!(
                                    clipped.clip_rect.bottom() <= properties_top,
                                    "size={size:?} points={points:?} clip={:?} header={header:?} addition={addition:?} properties_top={properties_top}",
                                    clipped.clip_rect
                                );
                                assert!(bottom.min(clipped.clip_rect.bottom()) <= properties_top);
                            }
                            let horizontal = (points[0].y - points[1].y).abs() < 0.1;
                            if horizontal
                                // The full-width panel divider is a separate
                                // one-point stroke, not an empty-grid rule.
                                && (stroke.width - 0.5).abs() < 0.1
                                && stroke.color == colors.grid_line.gamma_multiply(0.4)
                                && points[0].y > addition.bottom() + 22.0
                                && points[0].y < properties_top
                                && points[0].x <= header.left()
                                && points[1].x >= header.right() + 100.0
                            {
                                blank_rules += 1;
                                assert!(
                                    clipped.clip_rect.bottom() <= properties_top,
                                    "horizontal size={size:?} points={points:?} stroke={stroke:?} clip={:?} properties_top={properties_top}",
                                    clipped.clip_rect
                                );
                            }
                        }
                        egui::Shape::Rect(rectangle)
                            if rectangle.fill == colors.grid_header
                                && rectangle.rect.left() < header.left()
                                && rectangle.rect.right() <= header.left() + 1.0
                                && rectangle.rect.top() >= addition.bottom()
                                && rectangle.rect.height() > 22.0 =>
                        {
                            selector_backgrounds += 1;
                            assert!(clipped.clip_rect.bottom() <= properties_top);
                            assert!(rectangle.rect.bottom() <= properties_top);
                        }
                        _ => {}
                    }
                }
            }
            assert!(
                column_rules > 0,
                "the column rules must span actual field rows at {size:?}"
            );
            assert!(blank_rules > 0, "blank space must retain horizontal rules");
            assert_eq!(
                selector_backgrounds, 1,
                "one continuous gutter, no fake rows"
            );
            assert_eq!(
                harness
                    .state()
                    .0
                    .table_draft
                    .as_ref()
                    .unwrap()
                    .draft
                    .fields()
                    .len(),
                2
            );
            assert!(harness.state().1.is_empty());
        }
    }
}

#[test]
fn foreground_type_menu_can_extend_below_the_clipped_grid_without_changing_the_declaration() {
    let (mut state, field) = state();
    assert!(state.show_table_draft_design());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    let properties_top = harness.get_by_label("Field Properties").rect().top();
    let label = field_label(&harness.state().0, "table-draft-design-type", "Field1");
    harness
        .get_by_role_and_label(Role::ComboBox, &label)
        .click_accesskit();
    harness.run_steps(2);
    let option = harness.get_by_role_and_label(Role::Button, "Yes/No").rect();
    assert!(option.bottom() > properties_top);
    let text_clip = harness
        .output()
        .shapes
        .iter()
        .find_map(|clipped| {
            let mut leaves = Vec::new();
            paint_leaves(&clipped.shape, &mut leaves);
            leaves.into_iter().find_map(|shape| match shape {
                egui::Shape::Text(text) if text.galley.job.text == "Yes/No" => {
                    Some(clipped.clip_rect)
                }
                _ => None,
            })
        })
        .unwrap();
    assert!(
        text_clip.contains_rect(option),
        "option={option:?} clip={text_clip:?} properties_top={properties_top}"
    );
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields()[1].id(), field);
    assert_eq!(editor.draft.fields()[1].kind(), DraftFieldType::ShortText);
    assert!(harness.state().1.is_empty());
}
