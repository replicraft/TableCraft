//! Use the real Table command against resource-free presentation facts only.
use super::LAYOUT_SETTLE_STEPS;
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_ui::{FluentArgs, preview::Preview};

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

/// Exercise the real manual Design workflow using only synthetic source facts.
pub(super) fn design(harness: &mut Harness<'_, Preview>, mode: &str) {
    let localizer = harness.state().state().localizer.clone();
    assert!(harness.state().state().can_create_table_design());
    harness
        .get_by_role_and_label(Role::Button, &localizer.text("ribbon-table-design"))
        .click_accesskit();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    assert!(harness.state().state().table_draft_design_visible());
    assert!(!harness.state().state().can_save_table_draft());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "ID")
            .is_none()
    );
    if mode == "create-design" {
        return;
    }

    add_design_field(harness, "table-draft-short-text", "Name");
    if mode == "design-key" {
        add_design_field(harness, "table-draft-number", "ActualKey");
        harness
            .get_by_role_and_label(Role::Button, "ActualKey")
            .click_accesskit();
        harness.run_steps(LAYOUT_SETTLE_STEPS);
        assert!(harness.state().state().can_toggle_table_draft_primary_key());
        let revision = harness.state().state().table_draft_revision().unwrap();
        harness
            .get_by_role_and_label(Role::Button, &localizer.text("ribbon-primary-key"))
            .click_accesskit();
        harness.run_steps(LAYOUT_SETTLE_STEPS);
        assert_eq!(
            harness.state().state().table_draft_revision(),
            Some(revision + 1)
        );
        let selector = harness
            .get_by_role_and_label(Role::Button, "ActualKey")
            .rect();
        assert!(
            harness.output().shapes.iter().any(|shape| {
                let egui::Shape::Rect(image) = &shape.shape else {
                    return false;
                };
                image.brush.is_some()
                    && selector.contains_rect(image.rect)
                    && (image.rect.width() - 12.0).abs() < 1.0
            }),
            "the chosen manual key paints its key marker at the actual second field"
        );
    } else {
        assert_eq!(mode, "design-keyless-prompt");
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(LAYOUT_SETTLE_STEPS);
        harness
            .get_by_role_and_label(Role::Button, &localizer.text("table-draft-ok"))
            .click_accesskit();
        // The new modal needs its sizing/placement pass after the naming modal
        // disappears. Its accessibility nodes exist before its first paint.
        harness.run();
        assert!(harness.state().state().table_draft_save_open());
        assert!(!harness.state().state().table_draft_save_pending());
        assert!(
            harness
                .query_by_label(&localizer.text("table-draft-no-key-title"))
                .is_some()
        );
        let title = localizer.text("table-draft-no-key-title");
        assert!(
            harness.output().shapes.iter().any(|shape| {
                let egui::Shape::Text(text) = &shape.shape else {
                    return false;
                };
                text.galley.job.text == title
                    && shape
                        .clip_rect
                        .intersects(text.galley.rect.translate(text.pos.to_vec2()))
            }),
            "the primary-key decision must be painted, not only accessible"
        );
        for key in [
            "table-draft-property-yes",
            "table-draft-property-no",
            "table-draft-cancel",
        ] {
            assert!(
                harness
                    .query_by_role_and_label(Role::Button, &localizer.text(key))
                    .is_some()
            );
        }
    }
}

fn add_design_field(harness: &mut Harness<'_, Preview>, kind: &str, name: &str) {
    let localizer = harness.state().state().localizer.clone();
    harness
        .get_by_role_and_label(
            Role::Button,
            &localizer.text("table-draft-design-add-field"),
        )
        .click_accesskit();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness
        .get_by_role_and_label(Role::Button, &localizer.text(kind))
        .click_accesskit();
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness
        .get_by_role_and_label(Role::TextInput, &localizer.text("table-draft-field-name"))
        .type_text(name);
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness.key_press(Key::Enter);
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    let mut arguments = FluentArgs::new();
    arguments.set("field", name);
    assert!(
        harness
            .query_by_role_and_label(
                Role::Button,
                &localizer.format("table-draft-design-name", &arguments)
            )
            .is_some()
    );
    assert!(harness.state().state().can_save_table_draft());
}
