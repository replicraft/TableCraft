//! Use the real Table command against resource-free presentation facts only.
use super::LAYOUT_SETTLE_STEPS;
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_ui::preview::Preview;

pub(super) fn create(harness: &mut Harness<'_, Preview>, show_save_name: bool) {
    let localizer = harness.state().state().localizer.clone();
    let table = localizer.text("ribbon-table");
    // Some translations use the same label for the Ribbon command and the
    // navigation group's collapse button. The Ribbon is above navigation.
    let command = harness
        .get_all_by_role_and_label(Role::Button, &table)
        .min_by(|left, right| left.rect().top().total_cmp(&right.rect().top()))
        .expect("the table-ready scene exposes its Ribbon Table command");
    command.click_accesskit();
    harness.run_steps(LAYOUT_SETTLE_STEPS);

    if show_save_name {
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(LAYOUT_SETTLE_STEPS);
    }
}
