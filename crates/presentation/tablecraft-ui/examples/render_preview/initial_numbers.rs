//! Source-free Number captures use the same widgets and raw input as the desktop.
use super::{LAYOUT_SETTLE_STEPS, initial_properties};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_ui::preview::Preview;

pub(super) const FIELD_NAME: &str = "Amount";
pub(super) const RECORD_TEXT: &str = "12.5";

pub(super) fn edit(harness: &mut Harness<'_, Preview>, leave_size_menu_open: bool) {
    if leave_size_menu_open {
        let label = harness.state().state().localizer.text("design-field-size");
        harness
            .get_by_role_and_label(Role::ComboBox, &label)
            .click_accesskit();
        harness.run_steps(LAYOUT_SETTLE_STEPS);
        return;
    }

    initial_properties::choose(harness, "design-field-size", "table-draft-number-decimal");
    initial_properties::replace(
        harness,
        Role::TextInput,
        "table-draft-number-precision",
        "6",
    );
    initial_properties::replace(harness, Role::TextInput, "table-draft-number-scale", "2");
    initial_properties::choose(
        harness,
        "design-default-value",
        "table-draft-property-literal",
    );
    initial_properties::replace(
        harness,
        Role::MultilineTextInput,
        "table-draft-property-literal-input",
        "1.25",
    );
}
