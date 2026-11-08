//! Naming-only renderer interaction; no Enter, runtime capability or save is issued.
use super::LAYOUT_SETTLE_STEPS;
use egui::{Key, Shape, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_ui::preview::Preview;

pub(super) fn open(harness: &mut Harness<'_, Preview>) {
    let name = harness
        .state()
        .state()
        .database
        .as_ref()
        .expect("datasheet preview catalog")
        .tables
        .iter()
        .find(|table| table.local && !table.system)
        .expect("preview contains a local user table")
        .name
        .clone();

    // Close only the resource-free fixture's existing read-only object tabs.
    // Public close admission remains in force; this does not discard drafts.
    let tabs = harness.state().state().object_tabs();
    for tab in tabs {
        assert!(harness.state_mut().state_mut().close_object(&tab));
    }
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    assert!(harness.state().state().object_tabs().is_empty());

    // This explicit example-only gate exposes naming for visual inspection.
    // Normal Preview hosts still have no rename capability or enabled command.
    harness.state_mut().state_mut().table_rename_available = true;
    harness.get_by_role_and_label(Role::Button, &name).focus();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness.key_press(Key::F2);
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness.remove_cursor();

    let state = harness.state().state();
    assert!(state.table_rename_open());
    assert!(state.object_tabs().is_empty());
    assert!(state.data.is_none());
    let title = state.localizer.text("navigation-rename-table");
    let input = state.localizer.text("navigation-rename-name");
    assert!(harness.query_by_label(&title).is_some());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, &input)
            .value()
            .as_deref(),
        Some(name.as_str())
    );
    assert!(
        harness.output().shapes.iter().any(|shape| {
            let Shape::Text(text) = &shape.shape else {
                return false;
            };
            text.galley.job.text == title
                && shape
                    .clip_rect
                    .contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))
        }),
        "the actual rename heading is painted inside its viewport"
    );
}
