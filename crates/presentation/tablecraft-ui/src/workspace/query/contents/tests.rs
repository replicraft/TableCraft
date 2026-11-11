//! Result-local tokens follow complete publication and never become native targets.
use super::*;
use crate::workspace::query::tests::{self as query, harness};
use crate::{Cell, ContentsTarget, HorizontalAlignment, ObjectKind, ObjectTarget};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;

fn result(owner: u64) -> Data {
    Data {
        query_contents: QueryContentsBindings::new(
            owner,
            vec![QueryCollectionBinding {
                row: 0,
                column: 0,
                slot: 0,
            }],
            512,
        ),
        source_revision: None,
        row_ordinals: None,
        table: "Query results".into(),
        offset: 0,
        columns: vec!["Files".into()],
        rows: vec![vec![Cell {
            presentation: None,
            alignment: HorizontalAlignment::Left,
            text: "2 attachments".into(),
            kind: CellKind::Attachments {
                assigned: true,
                members: 2,
            },
        }]],
        has_more: false,
    }
}

#[test]
fn query_contents_tokens_reject_wrong_publications_and_follow_parked_object_ownership() {
    let mut state = query::state();
    let owner = state.active_object().unwrap();
    assert!(state.set_query_data(Some("SELECT Files".into()), result(10)));
    let target = QueryContentsTarget {
        result_id: 10,
        binding: 0,
    };
    assert!(state.can_view_query_contents(&target));
    assert!(!state.can_view_query_contents(&QueryContentsTarget {
        result_id: 9,
        binding: 0
    }));
    assert!(!state.can_view_query_contents(&QueryContentsTarget {
        result_id: 10,
        binding: 1
    }));
    assert!(!state.can_view_contents(&ContentsTarget {
        document_revision: 7,
        table: "Query results".into(),
        column: "Files".into(),
        parent_ordinal: 0
    }));
    assert!(!state.can_edit_cell_at(7, "Query results", "Files", 0));

    state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::SavedQuery {
                object_id: 9,
                name: "Typed prompts".into(),
            },
        })
        .unwrap();
    assert!(!state.can_view_query_contents(&target));
    assert!(state.activate_object(&owner));
    assert!(state.can_view_query_contents(&target));
    assert!(state.set_query_data(Some("SELECT NewFiles".into()), result(11)));
    assert!(!state.can_view_query_contents(&target));
    let current = QueryContentsTarget {
        result_id: 11,
        binding: 0,
    };
    assert!(state.can_view_query_contents(&current));
    assert!(state.close_object(&owner));
    assert!(!state.can_view_query_contents(&current));
}

#[test]
fn query_child_dialog_preserves_uninspected_members_and_rejects_late_result_callbacks() {
    let mut state = query::state();
    assert!(state.set_query_data(None, result(10)));
    let target = QueryContentsTarget {
        result_id: 10,
        binding: 0,
    };
    state.begin_query_contents(target, 20);
    let child = Data {
        query_contents: None,
        source_revision: None,
        row_ordinals: None,
        table: "Attachments".into(),
        offset: 0,
        columns: vec!["FileData".into()],
        rows: vec![vec![Cell {
            presentation: None,
            alignment: HorizontalAlignment::Left,
            text: "Not inspected".into(),
            kind: CellKind::Uninspected,
        }]],
        has_more: false,
    };
    state.set_query_contents(20, &target, child);
    assert_eq!(state.contents_request_id(), Some(20));
    let mut harness = harness(state);
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness.get_by_role_and_label(Role::Label, "Attachments");
    assert!(!harness.state().0.edit_dialog_open());
    assert_eq!(
        harness
            .state()
            .0
            .query_data
            .as_ref()
            .unwrap()
            .source_revision,
        None
    );
    harness.get_by_role_and_label(Role::Button, "Close").click();
    harness.run_steps(2);
    assert!(harness.state().0.contents_request_id().is_none());
    let late = query::result("late metadata");
    harness.state_mut().0.set_query_contents(20, &target, late);
    assert!(harness.state().0.contents_request_id().is_none());
}
