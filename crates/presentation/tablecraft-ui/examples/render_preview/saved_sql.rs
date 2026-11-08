//! Source-free visual scenarios, without a runtime review, database or native save.
mod status;
use super::LAYOUT_SETTLE_STEPS;
use egui::{Shape, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_ui::{SavedQuerySqlEligibility, preview::Preview};

const INSPECTION_ID: u64 = 710;
const SQL_INSPECTION_ID: u64 = 711;
const ORIGINAL: &str = "SELECT 42 AS Answer";
const PROPOSED: &str = "select 43 as [Answer];\n";

pub(super) fn is_mode(mode: &str) -> bool {
    matches!(
        mode,
        "none" | "editor" | "dirty" | "read-only" | "close" | "close-unavailable" | "close-stale"
    ) || status::is_mode(mode)
}

pub(super) fn prepare(harness: &mut Harness<'_, Preview>, mode: &str) {
    let target = harness
        .state()
        .state()
        .selected_saved_target()
        .expect("saved-query scene")
        .clone();
    {
        let state = harness.state_mut().state_mut();
        // Explicit example-only host fact. Normal Preview remains unavailable;
        // this synthetic inspection never issues native or save authority.
        state.saved_query_sql_edit_available = true;
        state.begin_saved_query_inspection(target.clone(), INSPECTION_ID);
        assert!(state.set_saved_query_details(INSPECTION_ID, target.clone(), vec![]));
        let request = state
            .begin_saved_query_sql_inspection(target, SQL_INSPECTION_ID)
            .unwrap();
        assert!(state.set_saved_query_sql_inspection(
            SQL_INSPECTION_ID,
            &request,
            Some(ORIGINAL.into()),
            SavedQuerySqlEligibility::QualifiedConstantSelect,
        ));
        if mode != "editor" && mode != "read-only" {
            assert!(state.set_saved_query_sql_text(PROPOSED.into()));
        }
        if mode == "read-only" || mode == "close-unavailable" {
            state.saved_query_sql_edit_available = false;
        }
        if mode.starts_with("close") {
            let tab = state.active_object().unwrap();
            assert!(state.request_saved_query_sql_close(&tab));
            if mode == "close-stale" {
                // A stale publication is a presentation test only. No document
                // source or native database is opened or replaced by this host.
                state.database.as_mut().unwrap().document_revision += 1;
            }
        }
    }
    if status::is_mode(mode) {
        status::prepare(harness, mode);
        return;
    }
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness.run();
    harness.remove_cursor();
    if mode.starts_with("close") {
        assert_close(harness, mode);
    } else {
        assert_editor(harness, mode);
    }
}

fn assert_editor(harness: &Harness<'_, Preview>, mode: &str) {
    let state = harness.state().state();
    let label = state.localizer.text("query-title");
    let editor = harness.get_by_role_and_label(Role::MultilineTextInput, &label);
    assert_eq!(editor.accesskit_node().is_disabled(), mode == "read-only");
    assert_eq!(
        editor.value().as_deref(),
        Some(if mode == "dirty" { PROPOSED } else { ORIGINAL })
    );
    assert_painted(harness, &label);
    if mode == "dirty" {
        assert_painted(harness, &state.localizer.text("saved-query-sql-unsaved"));
    }
    assert_eq!(state.saved_query_sql_dirty(), mode == "dirty");
}

fn assert_close(harness: &Harness<'_, Preview>, mode: &str) {
    let state = harness.state().state();
    assert!(state.saved_query_sql_close_open());
    assert!(state.saved_query_sql_dirty());
    let modal_rect = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new("saved_query_sql_close")))
        .expect("the actual saved SQL close modal owns a retained area");
    assert!(modal_rect.width() < harness.ctx.viewport_rect().width());
    assert!(modal_rect.height() < harness.ctx.viewport_rect().height());
    assert_painted_in(
        harness,
        &state.localizer.text("saved-query-close-title"),
        modal_rect,
    );
    for (key, disabled) in [
        ("saved-query-close-save", mode != "close"),
        ("saved-query-close-discard", mode == "close-stale"),
        ("saved-query-close-cancel", false),
    ] {
        let label = state.localizer.text(key);
        let buttons = harness
            .query_all_by_role_and_label(Role::Button, &label)
            .filter(|node| modal_rect.contains_rect(node.rect()))
            .collect::<Vec<_>>();
        assert_eq!(
            buttons.len(),
            1,
            "one {key} belongs to the actual close modal"
        );
        assert_eq!(buttons[0].accesskit_node().is_disabled(), disabled, "{key}");
        assert_painted_in(harness, &label, modal_rect);
    }
    assert_eq!(state.saved_query_sql().unwrap().1, PROPOSED);
}

fn assert_painted(harness: &Harness<'_, Preview>, label: &str) {
    assert_painted_in(harness, label, harness.ctx.viewport_rect());
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
        "expected text painted completely inside the viewport: {label}"
    );
}
