use crate::workspace::{
    Action, EditInput, InitialRecord, InsertInput, State,
    preview::{Preview, Scene},
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::table::{DraftDefaultMode, DraftFieldId, DraftFieldType, DraftNumberSize};

fn state() -> (State, DraftFieldId) {
    let mut preview = Preview::new(Scene::InitialTableDraft);
    let mut state = std::mem::take(preview.state_mut());
    state.table_save_available = true;
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    assert!(state.table_draft.as_mut().unwrap().commit_rename());
    let field = state.table_draft.as_ref().unwrap().draft.fields()[1].id();
    assert!(state.show_table_draft_design());
    state.table_draft.as_mut().unwrap().design.selected = Some(field);
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

fn replace(harness: &mut Harness<'_, (State, Vec<Action>)>, role: Role, label: &str, value: &str) {
    harness.get_by_role_and_label(role, label).focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Text(value.into()));
    harness.run_steps(2);
}

fn choose(harness: &mut Harness<'_, (State, Vec<Action>)>, label: &str, value: &str) {
    harness
        .get_by_role_and_label(Role::ComboBox, label)
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, value)
        .click_accesskit();
    harness.run_steps(2);
}

#[test]
fn rendering_properties_is_not_a_user_change_and_controls_write_exact_typed_choices() {
    let (state, _) = state();
    let mut harness = harness(state);
    assert_eq!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .property_revision(),
        0
    );
    replace(&mut harness, Role::TextInput, "Field Size", "0012");
    choose(&mut harness, "Required", "Yes");
    choose(&mut harness, "Allow Zero Length", "No");
    choose(&mut harness, "Default Value", "Literal");
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        "  exact Ω text  ",
    );
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    let snapshot = editor.draft.snapshot("Items").unwrap();
    assert_eq!(snapshot.fields()[1].short_text_units(), Some(12));
    assert_eq!(snapshot.fields()[1].required(), Some(true));
    assert_eq!(snapshot.fields()[1].allow_zero_length(), Some(false));
    assert!(
        matches!(snapshot.fields()[1].default_mode(), DraftDefaultMode::Literal(text) if text == "  exact Ω text  ")
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field Size")
            .value()
            .as_deref(),
        Some("0012")
    );
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
fn invalid_size_survives_view_tab_and_name_changes_until_explicit_correction() {
    let (state, field) = state();
    let mut harness = harness(state);
    replace(&mut harness, Role::TextInput, "Field Size", "not a size");
    assert!(!harness.state_mut().0.request_table_draft_save());
    assert!(
        !harness
            .state_mut()
            .0
            .set_table_draft_field_type(field, DraftFieldType::Number)
    );
    assert_eq!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()[1]
            .short_text_units(),
        Some(255)
    );
    assert!(harness.state_mut().0.show_table_draft_datasheet());
    harness.run_steps(2);
    let draft = harness.state().0.active_object().unwrap();
    let scratch = harness
        .state_mut()
        .0
        .prepare_object_open(crate::ObjectTarget {
            document_revision: draft.target.document_revision,
            kind: crate::ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert!(harness.state_mut().0.activate_object(&draft));
    assert!(harness.state_mut().0.show_table_draft_design());
    assert!(harness.state().0.object_tabs().contains(&scratch));
    harness
        .state_mut()
        .0
        .table_draft
        .as_mut()
        .unwrap()
        .draft
        .rename_field(field, "Renamed")
        .unwrap();
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field Size")
            .value()
            .as_deref(),
        Some("not a size")
    );
    replace(&mut harness, Role::TextInput, "Field Size", "24");
    assert!(harness.state_mut().0.request_table_draft_save());
    assert!(harness.state().1.is_empty());
}

#[test]
fn default_modes_keep_hidden_literal_text_and_multiline_input_never_becomes_a_formula() {
    let (state, _) = state();
    let mut harness = harness(state);
    choose(&mut harness, "Default Value", "Literal");
    let raw = "=Now()\nraw second line";
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        raw,
    );
    choose(&mut harness, "Default Value", "No default");
    assert!(matches!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()[1]
            .default_mode(),
        DraftDefaultMode::Absent
    ));
    choose(&mut harness, "Default Value", "Literal");
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Literal default")
            .value()
            .as_deref(),
        Some(raw)
    );
    choose(&mut harness, "Default Value", "NULL");
    assert!(matches!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()[1]
            .default_mode(),
        DraftDefaultMode::Null
    ));
    choose(&mut harness, "Default Value", "Type default");
    assert!(matches!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()[1]
            .default_mode(),
        DraftDefaultMode::TypeDefault
    ));
    assert!(harness.state().1.is_empty());
}

#[test]
fn oversized_default_paste_keeps_previous_text_and_blocks_save_until_corrected() {
    let (state, _) = state();
    let mut harness = harness(state);
    choose(&mut harness, "Default Value", "Literal");
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        "kept",
    );
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        &"x".repeat(4097),
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Literal default")
            .value()
            .as_deref(),
        Some("kept")
    );
    assert!(!harness.state_mut().0.request_table_draft_save());
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        "fixed",
    );
    assert!(harness.state_mut().0.request_table_draft_save());
    assert!(harness.state().1.is_empty());
}

#[test]
fn raw_property_change_invalidates_delete_and_discard_witnesses_without_model_change() {
    let (mut state, field) = state();
    let editor = state.table_draft.as_mut().unwrap();
    let position = editor.properties.ensure(&editor.draft, field).unwrap();
    let schema_revision = editor.draft.revision();
    assert!(state.request_table_draft_field_delete(field));
    let editor = state.table_draft.as_mut().unwrap();
    assert!(
        !editor
            .properties
            .set_size(&mut editor.draft, position, "invalid".into(), false)
    );
    assert_eq!(editor.draft.revision(), schema_revision);
    assert!(!state.confirm_table_draft_field_delete());
    assert_eq!(state.table_draft.as_ref().unwrap().draft.fields().len(), 2);
    let tab = state.active_object().unwrap();
    state.table_draft_discard_approved = Some((
        tab.clone(),
        schema_revision,
        state.table_draft_record_revision_for(&tab).unwrap(),
        state.table_draft_property_revision_for(&tab).unwrap(),
    ));
    let editor = state.table_draft.as_mut().unwrap();
    assert!(!editor.properties.set_size(
        &mut editor.draft,
        position,
        "new invalid input".into(),
        false
    ));
    assert!(!state.close_object(&tab));
    assert!(state.has_unsaved_table_drafts());
    assert!(state.table_draft_discard_approved.is_none());
}

#[test]
fn property_default_heap_is_included_in_initial_record_retention() {
    let (mut state, field) = state();
    let make = |state: &State| InitialRecord {
        snapshot: state
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .snapshot("Items")
            .unwrap(),
        inputs: vec![
            InsertInput::Omitted,
            InsertInput::Value(EditInput {
                text: "kept".into(),
                is_null: false,
            }),
        ],
        raw_text: vec![String::new(), "kept".into()],
    };
    let before = make(&state).retained_bytes().unwrap();
    let raw = "x".repeat(4096);
    state
        .table_draft
        .as_mut()
        .unwrap()
        .draft
        .set_field_default(field, DraftDefaultMode::Literal(raw))
        .unwrap();
    let after = make(&state).retained_bytes().unwrap();
    assert!(after >= before + 4096);
}

#[test]
fn compact_property_controls_remain_accessible_across_all_languages_and_themes() {
    for locale in crate::Locale::ALL {
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            for decimal in [false, true] {
                let (mut state, field) = state();
                let draft = &mut state.table_draft.as_mut().unwrap().draft;
                if decimal {
                    draft.set_field_type(field, DraftFieldType::Number).unwrap();
                    draft
                        .set_field_number_size(field, DraftNumberSize::Decimal)
                        .unwrap();
                }
                draft
                    .set_field_default(field, DraftDefaultMode::Literal("42".into()))
                    .unwrap();
                state.localizer = crate::Localizer::new(locale).unwrap();
                state.theme = theme;
                let mut controls = vec![
                    (
                        if decimal {
                            Role::ComboBox
                        } else {
                            Role::TextInput
                        },
                        "design-field-size",
                    ),
                    (Role::ComboBox, "design-required"),
                    (Role::ComboBox, "design-default-value"),
                    (
                        Role::MultilineTextInput,
                        "table-draft-property-literal-input",
                    ),
                ];
                if decimal {
                    controls.extend([
                        (Role::TextInput, "table-draft-number-precision"),
                        (Role::TextInput, "table-draft-number-scale"),
                    ]);
                } else {
                    controls.push((Role::ComboBox, "design-allow-zero-length"));
                }
                let controls = controls
                    .into_iter()
                    .map(|(role, key)| (role, state.localizer.text(key)))
                    .collect::<Vec<_>>();
                let general = state.localizer.text("design-general");
                let footer = state.localizer.text("table-draft-design-input-help");
                let mut harness = Harness::builder()
                    .with_size(egui::vec2(900.0, 580.0))
                    .build_ui_state(
                        |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                        (state, Vec::new()),
                    );
                crate::configure(&harness.ctx);
                harness.run_steps(3);
                let viewport =
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 580.0));
                let general_rect = harness.get_by_label(&general).rect();
                let footer_rect = harness.get_by_label(&footer).rect();
                for (role, label) in &controls {
                    let rect = harness.get_by_role_and_label(*role, label).rect();
                    assert!(
                        viewport.contains_rect(rect),
                        "{locale:?} {theme:?} decimal={decimal}: {rect:?}"
                    );
                    assert!(
                        rect.top() >= general_rect.bottom() && rect.bottom() <= footer_rect.top(),
                        "{locale:?} {theme:?} decimal={decimal}: control {rect:?} must fit between General {general_rect:?} and footer {footer_rect:?}"
                    );
                }
                assert!(harness.state().1.is_empty());
            }
        }
    }
}

#[test]
fn number_sizes_and_joint_decimal_text_update_only_the_unsaved_declaration() {
    let (mut state, field) = state();
    assert!(state.set_table_draft_field_type(field, DraftFieldType::Number));
    assert!(state.table_draft.as_mut().unwrap().record.set_input(
        field,
        InsertInput::Value(EditInput {
            text: "0007 Ω unchanged".into(),
            is_null: false,
        })
    ));
    assert!(
        state
            .table_draft
            .as_mut()
            .unwrap()
            .record
            .set_input(field, InsertInput::Omitted)
    );
    let mut harness = harness(state);
    for (label, expected) in [
        ("Byte", DraftNumberSize::Byte),
        ("Integer", DraftNumberSize::Integer),
        ("Long Integer", DraftNumberSize::LongInteger),
        ("Single", DraftNumberSize::Single),
        ("Double", DraftNumberSize::Double),
        ("Replication ID", DraftNumberSize::ReplicationId),
        ("Decimal", DraftNumberSize::Decimal),
    ] {
        choose(&mut harness, "Field Size", label);
        assert_eq!(
            harness
                .state()
                .0
                .table_draft
                .as_ref()
                .unwrap()
                .draft
                .fields()[1]
                .number_size(),
            Some(expected)
        );
        let editor = harness.state().0.table_draft.as_ref().unwrap();
        let snapshot = editor.draft.snapshot("Numbers").unwrap();
        assert_eq!(
            editor.record.raw_texts(&snapshot).unwrap()[1],
            "0007 Ω unchanged"
        );
        assert!(matches!(
            editor.record.inputs(&snapshot).unwrap()[1],
            InsertInput::Omitted
        ));
    }
    replace(&mut harness, Role::TextInput, "Scale", "007");
    replace(&mut harness, Role::TextInput, "Precision", "3");
    assert!(!harness.state_mut().0.request_table_draft_save());
    assert!(
        !harness
            .state_mut()
            .0
            .set_table_draft_field_type(field, DraftFieldType::ShortText)
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Precision")
            .value()
            .as_deref(),
        Some("3")
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Scale")
            .value()
            .as_deref(),
        Some("007")
    );
    assert!(harness.state_mut().0.show_table_draft_datasheet());
    harness.run_steps(2);
    assert!(harness.state_mut().0.show_table_draft_design());
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Precision")
            .value()
            .as_deref(),
        Some("3")
    );
    replace(&mut harness, Role::TextInput, "Precision", "0012");
    let constraints = harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .draft
        .fields()[1]
        .decimal_constraints()
        .unwrap();
    assert_eq!((constraints.precision(), constraints.scale()), (12, 7));
    assert!(
        !harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .properties
            .rejected()
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn replication_id_size_preserves_literal_but_blocks_save_until_default_mode_is_corrected() {
    let (mut state, field) = state();
    assert!(state.set_table_draft_field_type(field, DraftFieldType::Number));
    let mut harness = harness(state);
    choose(&mut harness, "Default Value", "Literal");
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        "kept literal",
    );
    choose(&mut harness, "Field Size", "Replication ID");
    assert!(!harness.state_mut().0.request_table_draft_save());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Literal default")
            .value()
            .as_deref(),
        Some("kept literal")
    );
    choose(&mut harness, "Default Value", "No default");
    assert!(
        !harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .properties
            .rejected()
    );
    choose(&mut harness, "Field Size", "Long Integer");
    choose(&mut harness, "Default Value", "Literal");
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Literal default")
            .value()
            .as_deref(),
        Some("kept literal")
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn property_cells_fill_the_sheet_align_as_rows_and_keep_rules_before_the_help_pane() {
    for size in [egui::vec2(1280.0, 820.0), egui::vec2(900.0, 580.0)] {
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            for decimal in [false, true] {
                let (mut state, field) = state();
                state.theme = theme;
                let draft = &mut state.table_draft.as_mut().unwrap().draft;
                if decimal {
                    draft.set_field_type(field, DraftFieldType::Number).unwrap();
                    draft
                        .set_field_number_size(field, DraftNumberSize::Decimal)
                        .unwrap();
                }
                draft
                    .set_field_default(field, DraftDefaultMode::Literal("42".into()))
                    .unwrap();
                let help = state.localizer.text("table-draft-property-help");
                let mut harness = Harness::builder().with_size(size).build_ui_state(
                    |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                    (state, Vec::new()),
                );
                crate::configure(&harness.ctx);
                harness.run_steps(3);
                let mut rows = vec![(
                    if decimal {
                        Role::ComboBox
                    } else {
                        Role::TextInput
                    },
                    "Field Size",
                )];
                if decimal {
                    rows.extend([(Role::TextInput, "Precision"), (Role::TextInput, "Scale")]);
                }
                rows.push((Role::ComboBox, "Required"));
                if !decimal {
                    rows.push((Role::ComboBox, "Allow Zero Length"));
                }
                rows.extend([
                    (Role::ComboBox, "Default Value"),
                    (Role::MultilineTextInput, "Literal default"),
                ]);
                let labels = rows.iter().map(|(_, label)| *label).collect::<Vec<_>>();
                let rows = rows
                    .iter()
                    .map(|(role, label)| harness.get_by_role_and_label(*role, label).rect())
                    .collect::<Vec<_>>();
                let first = rows[0];
                let last = *rows.last().unwrap();
                let sheet_left = harness.get_by_label("General").rect().left();
                let help_rect = harness.get_by_label(&help).rect();
                assert!(
                    first.width() > (help_rect.left() - sheet_left) * 0.5,
                    "value cells must use the property pane remainder: {size:?} {theme:?} {first:?} {help_rect:?}"
                );
                assert!(help_rect.left() > first.right());
                assert!(
                    help_rect.left() - first.right() < 40.0,
                    "no unused column between values and help"
                );
                for row in &rows {
                    assert!((row.left() - first.left()).abs() < 1.0);
                    assert!((row.right() - first.right()).abs() < 1.0);
                }
                let pixel = 1.0 / harness.ctx.pixels_per_point();
                for label in labels {
                    let label_rect = harness.get_by_role_and_label(Role::Label, label).rect();
                    assert!(
                        (label_rect.left() - sheet_left).abs() <= pixel,
                        "property labels start at the sheet edge: {label} {label_rect:?} sheet_left={sheet_left}"
                    );
                }
                let mut label_value_rule = false;
                let mut help_divider = false;
                let mut horizontal_rules = Vec::new();
                let mut vertical_rules = Vec::new();
                for clipped in &harness.output().shapes {
                    let mut leaves = Vec::new();
                    super::super::tests::paint_leaves(&clipped.shape, &mut leaves);
                    for shape in leaves {
                        let egui::Shape::LineSegment { points, .. } = shape else {
                            continue;
                        };
                        let vertical = (points[0].x - points[1].x).abs() < 0.1;
                        let top = points[0].y.min(points[1].y);
                        let bottom = points[0].y.max(points[1].y);
                        if vertical && bottom >= first.top() && top <= last.bottom() {
                            vertical_rules.push((*points, clipped.clip_rect));
                        }
                        if vertical && top <= first.top() + pixel && bottom >= last.bottom() - pixel
                        {
                            if (points[0].x - first.left()).abs() < 1.0 {
                                label_value_rule = true;
                                assert!(clipped.clip_rect.right() < help_rect.left());
                            }
                            if points[0].x > first.right() && points[0].x < help_rect.left() {
                                help_divider = true;
                            }
                        }
                        let horizontal = (points[0].y - points[1].y).abs() < 0.1;
                        if horizontal
                            && points[0].x <= sheet_left + pixel
                            && points[1].x >= first.right() - pixel
                            && points[1].x < help_rect.left()
                            && clipped.clip_rect.right() < help_rect.left()
                        {
                            horizontal_rules.push(points[0].y);
                        }
                    }
                }
                assert!(
                    label_value_rule,
                    "the label/value boundary must be visibly ruled: size={size:?} theme={theme:?} decimal={decimal} first={first:?} last={last:?} sheet_left={sheet_left} help={help_rect:?} vertical_rules={vertical_rules:?}"
                );
                assert!(
                    help_divider,
                    "a separate divider must precede the help pane"
                );
                horizontal_rules.sort_by(f32::total_cmp);
                horizontal_rules.dedup_by(|left, right| (*left - *right).abs() < pixel * 0.5);
                let mut previous_band = None;
                for row in &rows {
                    let covering = horizontal_rules
                        .windows(2)
                        .enumerate()
                        .filter_map(|(index, band)| {
                            (row.top() >= band[0] - pixel && row.bottom() <= band[1] + pixel)
                                .then_some(index)
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        covering.len(),
                        1,
                        "each control fits one fully ruled cell: size={size:?} theme={theme:?} decimal={decimal} row={row:?} horizontal_rules={horizontal_rules:?}"
                    );
                    let band = covering[0];
                    if let Some(previous) = previous_band {
                        assert_eq!(
                            band,
                            previous + 1,
                            "working controls occupy consecutive distinct cells, independent of control-frame insets"
                        );
                    }
                    previous_band = Some(band);
                }
                assert!(harness.state().1.is_empty());
            }
        }
    }
}
