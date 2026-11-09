use crate::workspace::{Action, DocumentView, Schema, State, schema::Field};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::{FieldAdditionDeclaration, FieldAdditionKind};

fn state() -> State {
    let mut state = crate::workspace::test_support::saved_table("Design naming", vec!["ID".into()]);
    state.field_addition_available = true;
    state.set_schema(Schema {
        table: "Table1".into(),
        declared_rows: 0,
        fields: vec![Field {
            name: "ID".into(),
            number: 0,
            native_type: 4,
            storage_bytes: 4,
            fixed: true,
            flags: 0,
            extended_flags: 0,
            scalar_type: Some("Int32".into()),
            display_type: Some("Number".into()),
            field_size: None,
            reader_nullable: Some(true),
            decimal_parameters: None,
            complex_type_id: None,
            properties: Vec::new(),
        }],
    });
    state.select_view(DocumentView::Design);
    state.data = None;
    state
}

#[test]
fn schema_only_design_uses_observed_choices_and_type_change_invalidates_old_review() {
    let mut state = state();
    let tab = state.active_object().unwrap();
    assert!(state.begin_field_addition_inspection(&tab, 80).is_ok());
    assert!(state.set_field_addition_availability(&tab, 80, true, true));
    assert!(state.begin_field_addition_name(
        &tab,
        "Label".into(),
        FieldAdditionDeclaration::ui_default(FieldAdditionKind::ShortText),
    ));
    let original = state.field_addition_review_request().unwrap();
    assert!(state.set_field_addition_kind(&tab, FieldAdditionKind::Number));
    let changed = state.field_addition_review_request().unwrap();
    assert_eq!(changed.column, "Label");
    assert_eq!(
        changed.declaration,
        FieldAdditionDeclaration::NumberLongZero
    );
    assert_eq!(changed.draft_revision, original.draft_revision + 1);
    assert!(!state.begin_field_addition_review(&original, 81));
    assert!(state.begin_field_addition_review(&changed, 81));
    assert!(!state.set_field_addition_kind(&tab, FieldAdditionKind::ShortText));
    assert!(state.release_field_addition_review(&changed, 81));
    assert!(state.discard_field_addition_name(&tab, changed.draft_revision));
    assert_eq!(state.schema.as_ref().unwrap().fields.len(), 1);
    assert!(state.data.is_none());
}

#[test]
fn blank_design_widgets_stage_one_local_proposal_without_inventing_a_stored_field() {
    let mut state = state();
    let tab = state.active_object().unwrap();
    assert!(state.begin_field_addition_inspection(&tab, 80).is_ok());
    assert!(state.set_field_addition_availability(&tab, 80, true, true));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 760.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "New field")
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::TextInput, "New field")
        .type_text("Pending Name");
    harness.run_steps(2);
    assert_eq!(
        harness.state().0.field_addition_name().unwrap().1,
        "Pending Name"
    );
    assert_eq!(harness.state().0.schema.as_ref().unwrap().fields.len(), 1);
    assert!(harness.state().1.is_empty());
    harness
        .get_by_role_and_label(Role::ComboBox, "Data Type")
        .click_accesskit();
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Currency")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_by_role_and_label(Role::Button, "Number")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness.state().0.field_addition_declaration(),
        Some(FieldAdditionDeclaration::NumberLongZero)
    );
    assert_eq!(
        harness.state().0.field_addition_name().unwrap().1,
        "Pending Name"
    );
    assert!(harness.state().1.is_empty());
    harness
        .get_by_role_and_label(Role::TextInput, "New field")
        .focus();
    harness.run_steps(1);
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().0.field_addition_open());
    assert_eq!(harness.state().0.schema.as_ref().unwrap().fields.len(), 1);
    assert!(harness.state().1.is_empty());
}
