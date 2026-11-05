//! Actual Home Find widgets, full-source worker and checked result publication.
use super::*;
use crate::workspace::test_support::{finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_testkit::fixtures::access::mutation_fixture;
use tablecraft_ui::{FindDirection, FindMatch, FindScope, FindTarget};

fn search(harness: &mut Harness<'_, Desktop>, pattern: &str) {
    harness.state_mut().state.open_find();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Find What:")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Find What:")
        .type_text(pattern);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Find Next")
        .click_accesskit();
    harness.run_steps(1);
}

#[test]
fn find_crosses_the_visible_page_and_preserves_it_after_not_found() {
    let rows = (1..=105).map(|id| (id, id * 10)).collect::<Vec<_>>();
    let bytes = mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0]);
    let (_directory, path, original, mut harness) = opened(bytes);
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 100);
    search(&mut harness, "105");
    finish_reads(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.offset, 104);
    assert_eq!(data.rows[0][0].text, "105");
    assert!(harness.state().state.find_window_open());

    search(&mut harness, "9999");
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().offset, 104);
    assert!(
        harness
            .query_by_label("No matching occurrence was found.")
            .is_some()
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn closing_pending_result_page_cancels_the_original_find_request() {
    let rows = (1..=105).map(|id| (id, id * 10)).collect::<Vec<_>>();
    let bytes = mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0]);
    let (_directory, _path, _original, mut harness) = opened(bytes);
    harness.state_mut().state.open_find();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Find What:")
        .type_text("105");
    harness.run_steps(2);

    // Dispatch without polling: cancellation must work even if the worker has
    // already finished its computation but the UI has not accepted the reply.
    let state = &harness.state().state;
    let data = state.data.as_ref().unwrap();
    let request = FindRequest {
        target: FindTarget {
            document_revision: state.database.as_ref().unwrap().document_revision,
            table: data.table.clone(),
        },
        scope: FindScope::Field(data.columns[0].name.clone()),
        pattern: "105".into(),
        matching: FindMatch::Whole,
        direction: FindDirection::All,
        start: Some(FindPosition {
            parent_ordinal: 0,
            column: data.columns[0].name.clone(),
            span: None,
        }),
        match_case: false,
        formatted: false,
        format_options: state.formatting.clone(),
    };
    assert!(state.can_find_next(&request));
    harness.state_mut().find_next(request.clone());
    assert!(harness.state().state.busy);
    let original_request_id = harness.state().generation;
    // Deliver the known position synchronously to hold the second-stage page
    // before publication. Its worker generation differs from the dialog ID.
    harness.state_mut().publish_found(
        original_request_id,
        runtime_request(request),
        Some(RuntimePosition {
            parent_ordinal: 104,
            column: "Id".into(),
            span: Some(runtime::FindSpan {
                start_utf16: 0,
                end_utf16: 3,
            }),
        }),
    );
    assert_ne!(harness.state().generation, original_request_id);
    let before = harness.state().state.data.as_ref().unwrap().offset;
    let request_id = harness.state_mut().state.close_find().unwrap();
    assert_eq!(request_id, original_request_id);
    harness.state_mut().cancel_find(request_id);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().offset, before);
    assert!(!harness.state().state.find_window_open());
}
