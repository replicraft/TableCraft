//! Source-free saved Number presentation, without a database or runtime review.
//!
//! The retained datasheet image informs the trailing header. No expanded menu,
//! naming or close capture exists; these scenes verify an adaptation, not Access
//! appearance or native write eligibility. Example-only facts never grant writes.
#[path = "number_column/returned.rs"]
mod returned;

use super::LAYOUT_SETTLE_STEPS;
use egui::{Shape, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_ui::{Data, ObjectKind, preview::Preview};

const INSPECTION_ID: u64 = 810;

pub(super) fn is_mode(mode: &str) -> bool {
    returned::is_mode(mode)
        || matches!(
            mode,
            "none" | "menu" | "name" | "close" | "close-unavailable"
        )
}

pub(super) fn prepare(harness: &mut Harness<'_, Preview>, mode: &str) {
    let tab = harness
        .state()
        .state()
        .active_object()
        .expect("datasheet owner");
    let ObjectKind::Table { name } = &tab.target.kind else {
        panic!("Number preview requires a saved table");
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
        state.number_column_available = true;
        assert!(
            state
                .begin_number_column_inspection(&tab, INSPECTION_ID)
                .is_ok()
        );
        assert!(state.set_number_column_availability(&tab, INSPECTION_ID, true));
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
    let number_label = harness.state().state().localizer.text("table-draft-number");
    let number = harness.get_by_role_and_label(Role::Button, &number_label);
    assert!(!number.accesskit_node().is_disabled());
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
                key != "table-draft-number"
            );
            assert_painted_in(harness, &label, harness.ctx.viewport_rect());
        }
        assert!(!harness.state().state().number_column_open());
    } else {
        number.click_accesskit();
        settle(harness);
        let state = harness.state().state();
        assert_eq!(state.number_column_name().unwrap().1, "Field1");
        let label = state.localizer.text("table-draft-field-name");
        let editor = harness.get_by_role_and_label(Role::TextInput, &label);
        assert!(!editor.accesskit_node().is_disabled());
        assert_eq!(editor.value().as_deref(), Some("Field1"));
        if returned::is_mode(mode) {
            returned::prepare(harness, mode);
        }
        if mode.starts_with("close") {
            let state = harness.state_mut().state_mut();
            assert!(state.request_number_column_close(&tab));
            if mode == "close-unavailable" {
                state.number_column_available = false;
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
    harness.run();
    harness.remove_cursor();
}

fn assert_close(harness: &Harness<'_, Preview>, unavailable: bool) {
    let state = harness.state().state();
    assert!(state.number_column_close_open());
    assert_eq!(state.number_column_name().unwrap().1, "Field1");
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new("number_column_close")))
        .expect("the actual Number confirmation owns a retained area");
    assert!(harness.ctx.viewport_rect().contains_rect(modal));
    assert_painted_in(
        harness,
        &state.localizer.text("datasheet-number-close-title"),
        modal,
    );
    for (key, disabled) in [
        ("datasheet-number-close-save", unavailable),
        ("datasheet-number-close-discard", false),
        ("datasheet-number-close-cancel", false),
    ] {
        let label = state.localizer.text(key);
        let buttons = harness
            .query_all_by_role_and_label(Role::Button, &label)
            .filter(|button| modal.contains_rect(button.rect()))
            .collect::<Vec<_>>();
        assert_eq!(
            buttons.len(),
            1,
            "one {key} belongs to the actual Number modal"
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
