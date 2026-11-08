//! Drive the actual pane control without a native window or database host.
use super::LAYOUT_SETTLE_STEPS;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_ui::preview::Preview;

pub(super) fn collapse(harness: &mut Harness<'_, Preview>) {
    let label = harness
        .state()
        .state()
        .localizer
        .text("navigation-collapse-pane");
    harness
        .get_by_role_and_label(Role::Button, &label)
        .click_accesskit();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    assert!(!harness.state().state().navigation_visible());
}
