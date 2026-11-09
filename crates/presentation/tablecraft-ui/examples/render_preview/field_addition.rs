//! Source-free saved-field presentation, without a database or native review.
//!
//! The retained datasheet image informs the trailing header. No expanded menu,
//! naming or close capture exists; these scenes verify an adaptation, not Access
//! appearance or native write eligibility. Example-only facts never grant writes.
#[path = "field_addition/returned.rs"]
mod returned;

use super::LAYOUT_SETTLE_STEPS;
use egui::{Shape, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::{FieldAdditionDeclaration, FieldAdditionKind};
use tablecraft_ui::{Data, ObjectKind, preview::Preview};

const INSPECTION_ID: u64 = 810;
const PRESENTATION_REQUEST_ID: u64 = 814;

pub(super) fn is_mode(mode: &str) -> bool {
    returned::is_mode(mode)
        || matches!(
            mode,
            "none"
                | "menu"
                | "name"
                | "close"
                | "close-unavailable"
                | "text-menu"
                | "text-name"
                | "text-close"
                | "text-close-unavailable"
                | "text-review"
        )
}

pub(super) fn prepare(harness: &mut Harness<'_, Preview>, mode: &str) {
    let text = mode.starts_with("text-");
    let mode = mode.strip_prefix("text-").unwrap_or(mode);
    let kind = if text {
        FieldAdditionKind::ShortText
    } else {
        FieldAdditionKind::Number
    };
    let tab = harness
        .state()
        .state()
        .active_object()
        .expect("datasheet owner");
    let ObjectKind::Table { name } = &tab.target.kind else {
        panic!("field preview requires a saved table");
    };
    {
        let state = harness.state_mut().state_mut();
        // Deliberately source-free and independent of the normal scene's wide
        // customer rows. This is a presentation fixture, not a native source.
        assert!(state.set_table_data(Data {
            source_revision: Some(tab.target.document_revision),
            row_ordinals: None,
            table: name.clone(),
            offset: 0,
            columns: vec!["ID".into()],
            rows: Vec::new(),
            has_more: false,
        }));
        state.field_addition_available = true;
        assert!(
            state
                .begin_field_addition_inspection(&tab, INSPECTION_ID)
                .is_ok()
        );
        assert!(state.set_field_addition_availability(&tab, INSPECTION_ID, true, text));
    }
    settle(harness);
    let label = harness
        .state()
        .state()
        .localizer
        .text("datasheet-click-to-add");
    let header = harness.get_by_role_and_label(Role::Button, &label);
    assert!(!header.accesskit_node().is_disabled());
    header.click_accesskit();
    settle(harness);
    let choice_key = match kind {
        FieldAdditionKind::Number => "table-draft-number",
        FieldAdditionKind::ShortText => "table-draft-short-text",
    };
    let choice_label = harness.state().state().localizer.text(choice_key);
    let choice = harness.get_by_role_and_label(Role::Button, &choice_label);
    assert!(!choice.accesskit_node().is_disabled());
    if mode == "menu" {
        for key in [
            "table-draft-number",
            "table-draft-short-text",
            "table-draft-currency",
        ] {
            let label = harness.state().state().localizer.text(key);
            let button = harness.get_by_role_and_label(Role::Button, &label);
            assert_eq!(
                button.accesskit_node().is_disabled(),
                key != "table-draft-number" && !(text && key == "table-draft-short-text")
            );
            assert_painted_in(harness, &label, harness.ctx.viewport_rect());
        }
        assert!(!harness.state().state().field_addition_open());
    } else {
        choice.click_accesskit();
        settle(harness);
        let state = harness.state().state();
        assert_eq!(state.field_addition_name().unwrap().1, "Field1");
        assert_eq!(
            state.field_addition_declaration(),
            Some(FieldAdditionDeclaration::ui_default(kind))
        );
        let label = state.localizer.text("table-draft-field-name");
        let editor = harness.get_by_role_and_label(Role::TextInput, &label);
        assert!(!editor.accesskit_node().is_disabled());
        assert_eq!(editor.value().as_deref(), Some("Field1"));
        if mode == "review" {
            let request = harness
                .state()
                .state()
                .field_addition_review_request()
                .expect("exact typed name snapshot");
            let state = harness.state_mut().state_mut();
            assert!(state.begin_field_addition_review(&request, PRESENTATION_REQUEST_ID));
            state.busy = true;
            assert!(state.field_addition_request_current(&request, PRESENTATION_REQUEST_ID));
            assert!(!state.can_review_field_addition());
            settle(harness);
            assert_review(harness, choice_key);
        }
        if returned::is_mode(mode) {
            returned::prepare(harness, mode);
        }
        if mode.starts_with("close") {
            let state = harness.state_mut().state_mut();
            assert!(state.request_field_addition_close(&tab));
            if mode == "close-unavailable" {
                state.field_addition_available = false;
            }
            settle(harness);
            assert_close(harness, mode == "close-unavailable");
        }
    }
    let data = harness.state().state().data.as_ref().unwrap();
    assert_eq!(
        data.columns.len(),
        1,
        "naming never publishes a stored column"
    );
    assert_eq!(data.columns[0].name, "ID");
    assert!(data.rows.is_empty(), "no synthetic record was inserted");
}

fn settle(harness: &mut Harness<'_, Preview>) {
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    // A pending review deliberately animates its spinner. Its layout still
    // settles in fixed frames; waiting for an idle repaint would never finish.
    if !harness.state().state().busy {
        harness.run();
    }
    harness.remove_cursor();
}

fn assert_close(harness: &Harness<'_, Preview>, unavailable: bool) {
    let state = harness.state().state();
    assert!(state.field_addition_close_open());
    assert_eq!(state.field_addition_name().unwrap().1, "Field1");
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new("field_addition_close")))
        .expect("the actual field confirmation owns a retained area");
    assert!(harness.ctx.viewport_rect().contains_rect(modal));
    assert_painted_in(
        harness,
        &state.localizer.text("datasheet-field-close-title"),
        modal,
    );
    for (key, disabled) in [
        ("datasheet-field-close-save", unavailable),
        ("datasheet-field-close-discard", false),
        ("datasheet-field-close-cancel", false),
    ] {
        let label = state.localizer.text(key);
        let buttons = harness
            .query_all_by_role_and_label(Role::Button, &label)
            .filter(|button| modal.contains_rect(button.rect()))
            .collect::<Vec<_>>();
        assert_eq!(
            buttons.len(),
            1,
            "one {key} belongs to the actual field modal"
        );
        assert_eq!(buttons[0].accesskit_node().is_disabled(), disabled, "{key}");
        assert_painted_in(harness, &label, modal);
    }
}

fn assert_painted_in(harness: &Harness<'_, Preview>, label: &str, owner: egui::Rect) {
    assert!(
        harness.output().shapes.iter().any(|shape| {
            let Shape::Text(text) = &shape.shape else {
                return false;
            };
            text.galley.job.text == label
                && shape
                    .clip_rect
                    .intersect(harness.ctx.viewport_rect())
                    .intersect(owner)
                    .contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))
        }),
        "expected text painted completely inside its owner: {label}"
    );
}

fn assert_review(harness: &Harness<'_, Preview>, choice_key: &str) {
    let state = harness.state().state();
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new("field_addition_review")))
        .expect("actual field review modal owns its area");
    assert!(harness.ctx.viewport_rect().contains_rect(modal));
    for key in [
        choice_key,
        "datasheet-field-reviewing",
        "editing-request-cancellation",
    ] {
        assert_painted_in(harness, &state.localizer.text(key), modal);
    }
    let cancel = state.localizer.text("editing-request-cancellation");
    let button = harness.get_by_role_and_label(Role::Button, &cancel);
    assert!(!button.accesskit_node().is_disabled());
    assert!(modal.contains_rect(button.rect()));
    assert_eq!(state.field_addition_name().unwrap().1, "Field1");
    assert!(
        state.field_addition_save_status().is_none(),
        "no synthetic native save actor is invented"
    );
}
