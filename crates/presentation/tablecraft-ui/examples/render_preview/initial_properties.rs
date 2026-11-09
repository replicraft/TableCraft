//! Drive the actual source-free property widgets before an offscreen capture.
use super::{INITIAL_FIELD_NAME, INITIAL_RECORD_TEXT, LAYOUT_SETTLE_STEPS};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_ui::{FluentArgs, preview::Preview};

pub(super) fn edit(harness: &mut Harness<'_, Preview>, leave_default_menu_open: bool) {
    replace(harness, Role::TextInput, "design-field-size", "40");
    choose(harness, "design-required", "table-draft-property-yes");
    choose(
        harness,
        "design-allow-zero-length",
        "table-draft-property-no",
    );
    choose(
        harness,
        "design-default-value",
        "table-draft-property-literal",
    );
    replace(
        harness,
        Role::MultilineTextInput,
        "table-draft-property-literal-input",
        "Ada",
    );
    if leave_default_menu_open {
        let label = harness
            .state()
            .state()
            .localizer
            .text("design-default-value");
        harness
            .get_by_role_and_label(Role::ComboBox, &label)
            .click_accesskit();
        harness.run_steps(LAYOUT_SETTLE_STEPS);
    }
}

pub(super) fn replace(harness: &mut Harness<'_, Preview>, role: Role, key: &str, value: &str) {
    let label = harness.state().state().localizer.text(key);
    replace_label(harness, role, &label, value);
}

fn replace_label(harness: &mut Harness<'_, Preview>, role: Role, label: &str, value: &str) {
    harness.get_by_role_and_label(role, label).focus();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Text(value.into()));
    harness.run_steps(LAYOUT_SETTLE_STEPS);
}

pub(super) fn choose(harness: &mut Harness<'_, Preview>, label: &str, choice: &str) {
    let label = harness.state().state().localizer.text(label);
    let choice = harness.state().state().localizer.text(choice);
    harness
        .get_by_role_and_label(Role::ComboBox, &label)
        .click_accesskit();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness
        .get_by_role_and_label(Role::Button, &choice)
        .click_accesskit();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
}

/// Description is authored through the real upper-grid widgets. No default
/// expression menu, save request or native storage operation is involved.
pub(super) fn description(harness: &mut Harness<'_, Preview>) {
    const DESCRIPTION: &str = "  Résumé 日本語 Now()  ";
    const IDENTIFIER_DESCRIPTION: &str = "Generated identifier";

    let localizer = &harness.state().state().localizer;
    let label = |field: &str| {
        let mut arguments = FluentArgs::new();
        arguments.set("field", field);
        localizer.format("table-draft-design-description", &arguments)
    };
    let field_label = label(INITIAL_FIELD_NAME);
    let identifier_label = label("ID");
    let revision = harness.state().state().table_draft_revision().unwrap();
    replace_label(harness, Role::MultilineTextInput, &field_label, DESCRIPTION);
    replace_label(
        harness,
        Role::MultilineTextInput,
        &identifier_label,
        IDENTIFIER_DESCRIPTION,
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, &field_label)
            .value()
            .as_deref(),
        Some(DESCRIPTION)
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, &identifier_label)
            .value()
            .as_deref(),
        Some(IDENTIFIER_DESCRIPTION)
    );
    assert!(harness.state().state().table_draft_revision().unwrap() >= revision + 2);

    assert!(harness.state_mut().state_mut().show_table_draft_datasheet());
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    let mut arguments = FluentArgs::new();
    arguments.set("field", INITIAL_FIELD_NAME);
    let record_label = harness
        .state()
        .state()
        .localizer
        .format("table-draft-record-field", &arguments);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, &record_label)
            .value()
            .as_deref(),
        Some(INITIAL_RECORD_TEXT)
    );
    assert!(harness.state_mut().state_mut().show_table_draft_design());
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, &field_label)
            .value()
            .as_deref(),
        Some(DESCRIPTION)
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, &identifier_label)
            .value()
            .as_deref(),
        Some(IDENTIFIER_DESCRIPTION)
    );
    assert!(!harness.state().state().table_draft_save_open());
}
