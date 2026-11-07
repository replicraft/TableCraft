//! Actual Home selection, checked pages and retained filter ownership.
use super::*;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::workspace::test_support::finish_save;
use crate::workspace::test_support::{finish_reads, focus_cell, opened, request_find};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::mutation_fixture;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tablecraft_ui::EditTarget;

fn source() -> Vec<u8> {
    let rows = (1..=205).map(|id| (id, id % 2)).collect::<Vec<_>>();
    mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0])
}

fn select_even_rows(harness: &mut Harness<'_, Desktop>) {
    focus_cell(harness, "Id", "2", "2");
    harness.key_press(egui::Key::ArrowRight);
    harness.run_steps(2);
    let target = harness.state().state.current_filter_selection().unwrap();
    assert_eq!(target.column, "Score");
    assert_eq!(target.parent_ordinal, 1);
    harness
        .get_by_role_and_label(Role::Button, "Selection")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Equals selected value")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(harness.state().state.table_filter_enabled());
    assert_eq!(harness.state().filter.as_ref().unwrap().matched_rows(), 102);
}

fn occurrence_rectangles(
    harness: &Harness<'_, Desktop>,
    cell: egui::Rect,
    fill: egui::Color32,
) -> Vec<egui::Rect> {
    harness
        .output()
        .shapes
        .iter()
        .filter_map(|shape| {
            let egui::Shape::Rect(rectangle) = &shape.shape else {
                return None;
            };
            (rectangle.fill == fill
                && cell.contains_rect(rectangle.rect)
                && shape.clip_rect.contains_rect(rectangle.rect)
                && rectangle.rect.width() < cell.width() / 2.0
                && rectangle.rect.height() < cell.height())
            .then_some(rectangle.rect)
        })
        .collect()
}

#[test]
fn selection_pages_toggle_and_clear_keep_physical_membership_and_original_bytes() {
    let (_directory, path, original, mut harness) = opened(source());
    select_even_rows(&mut harness);
    let filter_id = harness.state().filter.as_ref().unwrap().target().filter_id;
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 100);
    assert_eq!(data.parent_ordinal(0), Some(1));
    assert_eq!(data.parent_ordinal(99), Some(199));

    harness
        .get_by_role_and_label(Role::Button, "Page tools")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Next page ›")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.offset, 100);
    assert_eq!(data.rows.len(), 2);
    assert_eq!(data.parent_ordinal(0), Some(201));
    assert_eq!(data.rows[0][0].text, "202");
    assert!(!data.has_more);

    harness
        .get_by_role_and_label(Role::Button, "Toggle Filter")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().state.table_filter_enabled());
    assert_eq!(
        harness.state().filter.as_ref().unwrap().target().filter_id,
        filter_id
    );
    assert_eq!(
        harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .parent_ordinal(0),
        Some(0)
    );

    harness
        .get_by_role_and_label(Role::Button, "Advanced")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Clear All Filters")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.table_filter_count(), 0);
    assert!(!harness.state().state.table_filter_enabled());
    assert_eq!(harness.state().filter.as_ref().unwrap().filter_count(), 0);
    assert_eq!(
        harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .parent_ordinal(0),
        Some(0)
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn reviewed_edit_reopens_with_fresh_filter_identity_and_removes_the_changed_record() {
    let (_directory, path, original, mut harness) = opened(source());
    select_even_rows(&mut harness);
    let sort = SortSpec {
        column: "Id".into(),
        descending: true,
    };
    let target = harness.state().state.current_sort_target().unwrap();
    harness
        .state_mut()
        .sort_view(target, Some(sort.clone()), true);
    finish_reads(&mut harness);
    assert_eq!(
        harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .parent_ordinal(0),
        Some(203)
    );
    let old = harness.state().state.active_filter_target().unwrap();
    let target = EditTarget {
        document_revision: old.document_revision,
        table: old.table.clone(),
        column: "Score".into(),
        parent_ordinal: 203,
    };
    harness.state_mut().begin_edit(target);
    finish_reads(&mut harness);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .type_text("7");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Review change")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    harness
        .get_by_role_and_label(Role::Button, "Apply to original file")
        .click_accesskit();
    harness.run_steps(1);
    assert!(matches!(
        finish_save(&mut harness).outcome,
        SaveOutcome::Saved { .. }
    ));
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);

    let restored = harness.state().state.active_filter_target().unwrap();
    assert_ne!(restored.document_revision, old.document_revision);
    assert_ne!(restored.filter_id, old.filter_id);
    assert!(harness.state().state.table_filter_enabled());
    assert_eq!(harness.state().filter.as_ref().unwrap().matched_rows(), 101);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.parent_ordinal(0), Some(201));
    assert_eq!(data.rows[0][0].text, "202");
    assert_eq!(harness.state().state.table_sort(), Some(sort));
    assert!(data.rows.iter().all(|row| row[1].text == "0"));
    assert_ne!(std::fs::read(path).unwrap(), original);
}

#[test]
fn find_searches_filtered_members_and_keeps_the_filter_when_loading_a_later_hit() {
    let (_directory, path, original, mut harness) = opened(source());
    select_even_rows(&mut harness);
    let target = harness.state().state.active_filter_target().unwrap();
    focus_cell(&mut harness, "Id", "1", "2");

    request_find(&mut harness, "205");
    finish_reads(&mut harness);
    assert!(
        harness
            .query_by_label("No matching occurrence was found.")
            .is_some()
    );
    assert_eq!(harness.state().state.data.as_ref().unwrap().offset, 0);

    harness
        .get_by_role_and_label(Role::ComboBox, "Match:")
        .click_accesskit();
    harness.run_steps(2);
    harness.get_by_label("Any Part of Field").click_accesskit();
    harness.run_steps(2);
    request_find(&mut harness, "04");
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0][0].text,
        "104"
    );
    harness
        .get_by_role_and_label(Role::Button, "Find Next")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);

    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.offset, 101);
    assert_eq!(data.parent_ordinal(0), Some(203));
    assert_eq!(data.rows[0][0].text, "204");
    assert_eq!(
        harness.state().state.active_filter_target(),
        Some(target.clone())
    );
    assert!(harness.state().state.table_filter_enabled());
    assert!(harness.state().state.find_window_open());
    let PendingOperation::FindPage {
        position,
        filter: Some(filter),
        ..
    } = &harness.state().pending
    else {
        panic!("the accepted native hit retains its exact filtered page receipt");
    };
    assert_eq!(position.parent_ordinal, 203);
    assert_eq!(position.column, "Id");
    let span = position.span.as_ref().unwrap();
    assert_eq!((span.start_utf16, span.end_utf16), (1, 3));
    assert_eq!(filter.target, target);

    let header = harness
        .get_all_by_role(Role::ColumnHeader)
        .find(|node| node.accesskit_node().data().column_index() == Some(1))
        .unwrap()
        .rect();
    // The selected native column header supplies the real theme color. This
    // keeps the host proof independent of private palette/test-only APIs.
    // Its AX caption excludes the sort menu; the full painted header contains
    // that narrower field rectangle on the same row and left edge.
    let fill = harness
        .output()
        .shapes
        .iter()
        .rev()
        .find_map(|shape| {
            let egui::Shape::Rect(rectangle) = &shape.shape else {
                return None;
            };
            (rectangle.rect.contains_rect(header)
                && (rectangle.rect.left() - header.left()).abs() <= 1.0
                && (rectangle.rect.top() - header.top()).abs() <= 1.0
                && (rectangle.rect.bottom() - header.bottom()).abs() <= 1.0
                && rectangle.fill != egui::Color32::TRANSPARENT)
                .then_some(rectangle.fill)
        })
        .expect("actual selected column header fill");
    let mut arguments = tablecraft_ui::FluentArgs::new();
    arguments.set("column", "Id");
    arguments.set("row", "102");
    arguments.set("text", "204");
    let label = harness
        .state()
        .state
        .localizer
        .format("datasheet-cell-context", &arguments);
    let cell = harness.get_by_value(&label).rect();
    let rectangles = occurrence_rectangles(&harness, cell, fill);
    assert_eq!(
        rectangles.len(),
        2,
        "only the two matched digits are painted"
    );
    let text = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| {
            let egui::Shape::Text(text) = &shape.shape else {
                return None;
            };
            let bounds = text.galley.rect.translate(text.pos.to_vec2());
            (text.galley.job.text == "204" && cell.contains_rect(bounds)).then_some(bounds)
        })
        .expect("unchanged complete native cell text");
    assert!(
        rectangles
            .iter()
            .all(|rectangle| rectangle.left() > text.left() + 1.0)
    );

    // A real user selection and another filtered page cannot inherit the span.
    focus_cell(&mut harness, "Score", "102", "0");
    assert_eq!(
        harness
            .state()
            .state
            .current_filter_selection()
            .unwrap()
            .column,
        "Score"
    );
    // Selection observation follows cell painting in the focus-event frame.
    // Inspect the settled renderer output, rather than that previous frame.
    harness.run_steps(2);
    assert!(occurrence_rectangles(&harness, cell, fill).is_empty());
    harness
        .get_by_role_and_label(Role::Button, "Page tools")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "‹ Previous page")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_ne!(harness.state().state.data.as_ref().unwrap().offset, 101);
    assert!(occurrence_rectangles(&harness, cell, fill).is_empty());
    assert_eq!(harness.state().state.active_filter_target(), Some(target));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn native_sort_pages_find_and_clear_retain_exact_physical_rows() {
    let (_directory, path, original, mut harness) = opened(source());
    select_even_rows(&mut harness);
    let target = harness.state().state.current_sort_target().unwrap();
    let sort = SortSpec {
        column: "Id".into(),
        descending: true,
    };
    harness
        .state_mut()
        .sort_view(target, Some(sort.clone()), true);
    finish_reads(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.parent_ordinal(0), Some(203));
    assert_eq!(data.parent_ordinal(99), Some(5));
    assert_eq!(data.row_index(203), Some(0));
    assert_eq!(harness.state().state.table_sort(), Some(sort.clone()));

    focus_cell(&mut harness, "Id", "1", "204");
    request_find(&mut harness, "2");
    finish_reads(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.offset, 101);
    assert_eq!(data.parent_ordinal(0), Some(1));
    assert_eq!(data.rows[0][0].text, "2");
    harness.state_mut().state.close_find();
    harness.run_steps(2);

    let target = harness.state().state.active_filter_target().unwrap();
    harness.state_mut().clear_table_filter(target);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.table_sort(), Some(sort));
    assert_eq!(harness.state().state.table_filter_count(), 0);
    assert!(!harness.state().state.table_filter_enabled());
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.parent_ordinal(0), Some(204));
    assert_eq!(data.rows[0][0].text, "205");

    let target = harness.state().state.current_sort_target().unwrap();
    harness.state_mut().sort_view(target, None, false);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.table_sort(), None);
    assert_eq!(
        harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .parent_ordinal(0),
        Some(0)
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}
