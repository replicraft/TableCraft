//! The normal saved Design widgets with explicitly source-free availability.
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_ui::preview::Preview;

const INSPECTION_ID: u64 = 841;
const REVIEW_REQUEST_ID: u64 = 842;

pub(super) fn is_mode(mode: &str) -> bool {
    matches!(
        mode,
        "design-blank"
            | "design-name"
            | "design-number-name"
            | "design-menu"
            | "design-duplicate"
            | "design-review"
    )
}

pub(super) fn prepare(harness: &mut Harness<'_, Preview>, mode: &str) {
    let tab = harness
        .state()
        .state()
        .active_object()
        .expect("saved Design owner");
    {
        let state = harness.state_mut().state_mut();
        let schema = state.schema.as_mut().expect("Design metadata scene");
        // No user records or native source back this illustrative declaration.
        schema.declared_rows = 0;
        schema.fields.truncate(1);
        state.data = None;
        state.field_addition_available = true;
        assert!(
            state
                .begin_field_addition_inspection(&tab, INSPECTION_ID)
                .is_ok()
        );
        assert!(state.set_field_addition_availability(
            &tab,
            INSPECTION_ID,
            true,
            true,
            Default::default()
        ));
    }
    super::settle(harness);
    let t = &harness.state().state().localizer;
    let name_label = t.text("design-new-field");
    let type_label = t.text("design-data-type");
    if mode == "design-menu" || mode == "design-number-name" {
        harness
            .get_by_role_and_label(Role::ComboBox, &type_label)
            .click();
        super::settle(harness);
        if mode == "design-menu" {
            return;
        }
        let number = harness.state().state().localizer.text("table-draft-number");
        harness.get_by_role_and_label(Role::Button, &number).click();
        super::settle(harness);
    }
    if mode == "design-blank" {
        assert!(!harness.state().state().field_addition_open());
        return;
    }

    let name = if mode == "design-duplicate" {
        // NO_CLONE: The proposal owns text while the separate unchanged schema
        // remains available to test duplicate-name admission in the real widgets.
        harness.state().state().schema.as_ref().unwrap().fields[0]
            .name
            .clone()
    } else if mode == "design-number-name" {
        "Score".into()
    } else {
        "Notes".into()
    };
    harness
        .get_by_role_and_label(Role::TextInput, &name_label)
        .focus();
    super::settle(harness);
    harness
        .get_by_role_and_label(Role::TextInput, &name_label)
        .type_text(&name);
    super::settle(harness);
    let state = harness.state().state();
    assert_eq!(state.schema.as_ref().unwrap().fields.len(), 1);
    assert_eq!(state.field_addition_name().unwrap().1, name);
    assert_eq!(
        state.field_addition_declaration().unwrap().kind(),
        if mode == "design-number-name" {
            FieldAdditionKind::Number
        } else {
            FieldAdditionKind::ShortText
        }
    );
    if mode == "design-review" {
        let request = state
            .field_addition_review_request()
            .expect("staged source-free request");
        let state = harness.state_mut().state_mut();
        assert!(state.begin_field_addition_review(&request, REVIEW_REQUEST_ID));
        state.busy = true;
        super::settle(harness);
    }
}
