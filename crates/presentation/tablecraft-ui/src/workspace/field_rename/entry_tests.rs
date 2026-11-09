//! Actual heading and Design entry admission, before any native source proof.
use super::test_support::{harness, naming};
use crate::{Action, Field, FieldProperty, FieldPropertyValue, Schema, State};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};

fn unopened_editor() -> State {
    let (mut state, _) = naming();
    state.prepare_clean_field_rename_close().unwrap().complete();
    assert!(!state.field_rename_open());
    state
}

fn choose_rename(harness: &mut Harness<'_, (State, Vec<Action>)>, label: &str) {
    harness.get_by_label(label).click();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    let title = harness.state().0.localizer.text("field-rename-title");
    harness
        .get_by_role_and_label(Role::Button, &title)
        .click_accesskit();
    // One frame emits the readonly request. Deliberately do not settle another
    // source-free frame before the host has bound that outgoing inspection.
    harness.run_steps(1);
}

fn assert_inspection(harness: &Harness<'_, (State, Vec<Action>)>) {
    let [Action::InspectFieldRename { target }] = harness.state().1.as_slice() else {
        panic!("exactly one source inspection, without review or save");
    };
    assert_eq!(target.column, "Score");
    assert_eq!(target.native_number, 17);
    assert_eq!(target.tab.target.document_revision, 7);
    assert_eq!(target.tab, harness.state().0.active_object().unwrap());
    assert!(!harness.state().0.can_edit_field_rename());
    assert_eq!(
        harness.state().0.field_rename_proposal().unwrap().1,
        "Score"
    );
}

#[test]
fn captioned_datasheet_keyboard_menu_binds_true_name_and_not_filter_chevron() {
    let mut harness = harness(unopened_editor());
    let mut arguments = crate::FluentArgs::new();
    arguments.set("column", "Score");
    let menu = harness
        .state()
        .0
        .localizer
        .format("datasheet-column-menu", &arguments);
    let caption_rect = harness.get_by_label("Friendly").rect();
    let filter_rect = harness.get_by_role_and_label(Role::Button, &menu).rect();
    assert!(caption_rect.right() <= filter_rect.left());
    harness.get_by_role_and_label(Role::Button, &menu).click();
    harness.run_steps(2);
    assert!(!harness.state().0.field_rename_open());
    assert!(harness.state().1.is_empty());
    let filter = harness.state().0.localizer.text("filtering-field-title");
    harness.get_by_role_and_label(Role::Button, &filter);
    harness.key_press(Key::Escape);
    harness.run_steps(2);

    choose_rename(&mut harness, "Friendly");
    assert_inspection(&harness);
    let column = &harness.state().0.data.as_ref().unwrap().columns[1];
    assert_eq!(column.name, "Score");
    assert_eq!(column.caption.as_deref(), Some("Friendly"));
    assert_eq!(column.native_number, Some(17));
}

#[test]
fn saved_design_name_cell_uses_actual_number_and_keeps_declarations_readonly() {
    let mut state = unopened_editor();
    let tab = state.active_object().unwrap();
    assert!(state.prepare_object_design(&tab).unwrap().needs_load);
    state.set_schema(Schema {
        table: "Table1".into(),
        declared_rows: 0,
        fields: vec![Field {
            name: "Score".into(),
            number: 17,
            native_type: 4,
            storage_bytes: 4,
            fixed: true,
            flags: 3,
            extended_flags: 0,
            scalar_type: Some("Int32".into()),
            display_type: Some("Number".into()),
            field_size: Some("Long Integer".into()),
            reader_nullable: Some(true),
            decimal_parameters: None,
            complex_type_id: None,
            properties: vec![
                FieldProperty {
                    name: "Required",
                    value: FieldPropertyValue::Boolean(false),
                },
                FieldProperty {
                    name: "DefaultValue",
                    value: FieldPropertyValue::Text("0".into()),
                },
            ],
        }],
    });
    let mut harness = harness(state);
    // The declared type is an inert label, not the draft-table type picker.
    harness.get_by_role_and_label(Role::Label, "Number");
    // A settled frame need not emit another incremental AccessKit update.
    // Query the harness's actual retained tree, as other widget tests do.
    let property_label = harness.state().0.localizer.text("design-default-value");
    let property_value = harness
        .get_all_by_role(Role::Label)
        .find(|value| {
            let node = value.accesskit_node();
            node.value().as_deref() == Some("0")
                && node
                    .labelled_by()
                    .any(|label| label.value().as_deref() == Some(property_label.as_str()))
        })
        .expect("actual inert default property value with its own label");
    assert!(
        !property_value
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::SetValue)
    );
    choose_rename(&mut harness, "Score");
    assert_inspection(&harness);
    let field = &harness.state().0.schema.as_ref().unwrap().fields[0];
    assert_eq!(field.number, 17);
    assert_eq!(field.name, "Score");
    assert_eq!(field.native_type, 4);
    assert_eq!(field.display_type.as_deref(), Some("Number"));
    assert!(matches!(
        field.properties[0].value,
        FieldPropertyValue::Boolean(false)
    ));
    assert!(matches!(&field.properties[1].value, FieldPropertyValue::Text(value) if value == "0"));
    assert!(!harness.state().0.table_draft_design_visible());
}
