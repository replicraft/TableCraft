//! Adapted query result/contents frames; the preview has no backend or runtime handle.
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_ui::preview::Preview;

pub(super) fn is_mode(mode: &str) -> bool {
    matches!(mode, "none" | "parent" | "contents" | "empty")
}

pub(super) fn prepare(harness: &mut Harness<'_, Preview>, mode: &str) {
    assert!(harness.state_mut().prepare_query_recordsets());
    harness.run_steps(3);
    if mode == "parent" {
        return;
    }
    let mut arguments = tablecraft_ui::FluentArgs::new();
    arguments.set("count", if mode == "empty" { 0 } else { 2 });
    let needle = harness
        .state()
        .state()
        .localizer
        .format("collections-attachment-summary", &arguments);
    let cell = harness
        .query_all_by_role(Role::Label)
        .find(|node| {
            let node = node.accesskit_node();
            node.label().is_some_and(|text| text.contains(&needle))
                || node.value().is_some_and(|text| text.contains(&needle))
        })
        .expect("actual synthetic query collection summary");
    cell.click_secondary();
    harness.run_steps(2);
    let label = harness
        .state()
        .state()
        .localizer
        .text("datasheet-view-contents");
    harness
        .get_by_role_and_label(Role::Button, &label)
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().state().contents_request_id().is_some());
    assert_eq!(
        harness
            .state()
            .state()
            .query_data
            .as_ref()
            .unwrap()
            .source_revision,
        None
    );
}
