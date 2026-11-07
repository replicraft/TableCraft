//! Drive the actual source-free property widgets before an offscreen capture.
use super::LAYOUT_SETTLE_STEPS;
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_ui::preview::Preview;

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

fn replace(harness: &mut Harness<'_, Preview>, role: Role, key: &str, value: &str) {
    let label = harness.state().state().localizer.text(key);
    harness.get_by_role_and_label(role, &label).focus();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Text(value.into()));
    harness.run_steps(LAYOUT_SETTLE_STEPS);
}

fn choose(harness: &mut Harness<'_, Preview>, label: &str, choice: &str) {
    let localizer = harness.state().state().localizer.clone();
    harness
        .get_by_role_and_label(Role::ComboBox, &localizer.text(label))
        .click_accesskit();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness
        .get_by_role_and_label(Role::Button, &localizer.text(choice))
        .click_accesskit();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
}
