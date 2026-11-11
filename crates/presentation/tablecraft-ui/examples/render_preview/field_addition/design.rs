//! The normal saved Design widgets with explicitly source-free availability.
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::sync::Arc;
use tablecraft_builders::table::{
    FieldAdditionKind, TypedFieldAdditionAvailability, TypedFieldAdditionDeclaration,
};
use tablecraft_schema::ScalarType;
use tablecraft_ui::preview::Preview;

const INSPECTION_ID: u64 = 841;
const REVIEW_REQUEST_ID: u64 = 842;

pub(super) fn is_mode(mode: &str) -> bool {
    matches!(
        mode,
        "design-currency"
            | "design-decimal"
            | "design-typed-menu"
            | "design-blank"
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
            if matches!(
                mode,
                "design-currency" | "design-decimal" | "design-typed-menu"
            ) {
                let declarations = [
                    TypedFieldAdditionDeclaration::fixed(ScalarType::Currency).unwrap(),
                    TypedFieldAdditionDeclaration::decimal(18, 0).unwrap(),
                ];
                Arc::from(
                    declarations.map(|declaration| TypedFieldAdditionAvailability {
                        declaration,
                        maximum_name_utf16_units: 64,
                    }),
                )
            } else {
                Default::default()
            }
        ));
    }
    super::settle(harness);
    let t = &harness.state().state().localizer;
    let name_label = t.text("design-new-field");
    let type_label = t.text("design-data-type");
    if matches!(
        mode,
        "design-menu"
            | "design-number-name"
            | "design-currency"
            | "design-decimal"
            | "design-typed-menu"
    ) {
        harness
            .get_by_role_and_label(Role::ComboBox, &type_label)
            .click();
        super::settle(harness);
        if matches!(mode, "design-menu" | "design-typed-menu") {
            return;
        }
        let number = harness
            .state()
            .state()
            .localizer
            .text(if mode == "design-currency" {
                "table-draft-currency"
            } else {
                "table-draft-number"
            });
        harness.get_by_role_and_label(Role::Button, &number).click();
        super::settle(harness);
    }
    if mode == "design-decimal" {
        let size = harness.state().state().localizer.text("design-field-size");
        harness
            .get_by_role_and_label(Role::ComboBox, &size)
            .click_accesskit();
        super::settle(harness);
        let decimal = harness
            .state()
            .state()
            .localizer
            .text("table-draft-number-decimal");
        harness
            .get_by_role_and_label(Role::Button, &decimal)
            .click_accesskit();
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
    } else if mode == "design-currency" {
        "Amount".into()
    } else if matches!(mode, "design-number-name" | "design-decimal") {
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
        if mode == "design-currency" {
            FieldAdditionKind::Currency
        } else if matches!(mode, "design-number-name" | "design-decimal") {
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
