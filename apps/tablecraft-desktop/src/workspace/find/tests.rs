//! Actual Home Find widgets, full-source worker and checked result publication.
use super::*;
use crate::workspace::test_support::{finish_reads, opened, request_find};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_testkit::fixtures::access::mutation_fixture;
use tablecraft_ui::{FindDirection, FindMatch, FindScope, FindTarget};

#[test]
fn find_crosses_the_visible_page_and_preserves_it_after_not_found() {
    let rows = (1..=105).map(|id| (id, id * 10)).collect::<Vec<_>>();
    let bytes = mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0]);
    let (_directory, path, original, mut harness) = opened(bytes);
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 100);
    request_find(&mut harness, "105");
    finish_reads(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.offset, 104);
    assert_eq!(data.rows[0][0].text, "105");
    assert!(harness.state().state.find_window_open());

    request_find(&mut harness, "9999");
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

#[test]
fn borrowed_reply_comparison_matches_every_request_coordinate_and_option() {
    let request = FindRequest {
        target: FindTarget {
            document_revision: 7,
            table: "Items".into(),
        },
        scope: FindScope::Field("Name".into()),
        pattern: "needle".into(),
        matching: FindMatch::Whole,
        direction: FindDirection::Down,
        start: Some(FindPosition {
            parent_ordinal: 13,
            column: "Name".into(),
            span: Some(FindSpan {
                start_utf16: 2,
                end_utf16: 8,
            }),
        }),
        match_case: true,
        formatted: true,
        format_options: Default::default(),
    };
    let reply = runtime_request(request.clone());
    assert!(runtime_request_matches(&request, &reply));

    let mismatches: [fn(&mut RuntimeRequest); 15] = [
        |reply| reply.target.document_revision += 1,
        |reply| reply.target.table.push('x'),
        |reply| reply.scope = runtime::FindScope::Table,
        |reply| reply.scope = runtime::FindScope::Field("Other".into()),
        |reply| reply.pattern.push('x'),
        |reply| reply.matching = runtime::FindMatch::Any,
        |reply| reply.direction = runtime::FindDirection::Up,
        |reply| reply.start = None,
        |reply| reply.start.as_mut().unwrap().parent_ordinal += 1,
        |reply| reply.start.as_mut().unwrap().column.push('x'),
        |reply| reply.start.as_mut().unwrap().span = None,
        |reply| {
            reply
                .start
                .as_mut()
                .unwrap()
                .span
                .as_mut()
                .unwrap()
                .start_utf16 += 1;
        },
        |reply| {
            reply
                .start
                .as_mut()
                .unwrap()
                .span
                .as_mut()
                .unwrap()
                .end_utf16 += 1;
        },
        |reply| reply.match_case = false,
        |reply| reply.formatted = false,
    ];
    for (index, mismatch) in mismatches.into_iter().enumerate() {
        let mut changed = reply.clone();
        mismatch(&mut changed);
        assert!(!runtime_request_matches(&request, &changed), "case {index}");
    }
    let mut changed = reply;
    changed.format_options.system_locale = Some("fr-FR".into());
    assert!(!runtime_request_matches(&request, &changed));

    for matching in [FindMatch::Any, FindMatch::Whole, FindMatch::Start] {
        for direction in [FindDirection::All, FindDirection::Up, FindDirection::Down] {
            for scope in [FindScope::Table, FindScope::Field("Name".into())] {
                let mut current = request.clone();
                current.matching = matching;
                current.direction = direction;
                current.scope = scope;
                for start in [None, request.start.clone()] {
                    current.start = start;
                    let reply = runtime_request(current.clone());
                    assert!(runtime_request_matches(&current, &reply));
                }
                current.start = Some(FindPosition {
                    parent_ordinal: 13,
                    column: "Name".into(),
                    span: None,
                });
                let reply = runtime_request(current.clone());
                assert!(runtime_request_matches(&current, &reply));
            }
        }
    }
}
