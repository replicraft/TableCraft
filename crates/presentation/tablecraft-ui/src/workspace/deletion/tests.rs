use super::*;
use crate::{Action, Data, Database, Table};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn state() -> (State, DeleteTarget) {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        document_revision: 7,
        title_path: None,
        title: "Deletion".into(),
        format: "ACE 16".into(),
        tables: vec![Table {
            local: true,
            name: "People".into(),
            system: false,
            storage: "Local".into(),
        }],
        queries: Vec::new(),
    });
    state.set_table_data(Data {
        row_ordinals: None,
        source_revision: Some(7),
        table: "People".into(),
        offset: 100,
        columns: vec!["Name".into()],
        rows: vec![vec!["Ada".into()]],
        has_more: false,
    });
    let target = DeleteTarget {
        document_revision: 7,
        table: "People".into(),
        parent_ordinal: 100,
    };
    (state, target)
}

fn review(target: DeleteTarget) -> DeleteReview {
    DeleteReview {
        id: 30,
        target,
        columns: vec!["Name".into()],
        row: vec!["Ada".into()],
        deleted_rows: 1,
    }
}

#[test]
fn delete_requires_source_bound_native_user_row_and_retains_save_owner() {
    let (mut state, target) = state();
    assert_eq!(state.current_delete_target(), Some(target.clone()));
    state.begin_delete(target.clone(), 1);
    assert!(!state.set_delete_review(2, review(target.clone())));
    assert!(state.set_delete_review(1, review(target.clone())));
    assert!(state.can_apply_delete(30));
    assert!(!state.can_apply_delete(31));
    assert!(state.current_insert_target().is_none());

    state.begin_delete_save(30, 40);
    assert!(!state.close_delete());
    state.set_delete_save_outcome(41, EditSaveOutcome::Saved("stale".into()));
    assert!(state.delete_save_pending());
    state.set_delete_save_outcome(
        40,
        EditSaveOutcome::Retained {
            message: "Current outcome unknown; previous save committed.".into(),
            can_recover: true,
            can_release: false,
        },
    );
    assert!(!state.close_delete());
    state.invalidate_edit();
    assert!(state.deletion_dialog_open());
    state.begin_delete_resolution(40, 42);
    state.set_delete_save_outcome(
        42,
        EditSaveOutcome::Failed("Diagnosis retained literally.".into()),
    );
    assert!(state.close_delete());

    state.database.as_mut().unwrap().tables[0].system = true;
    assert!(!state.can_delete_row(&target));
    state.database.as_mut().unwrap().tables[0].system = false;
    state.data.as_mut().unwrap().source_revision = None;
    assert!(!state.can_delete_row(&target));
    state.data.as_mut().unwrap().source_revision = Some(7);
    state.view = DocumentView::Query;
    assert!(!state.can_delete_row(&target));
}

#[test]
fn actual_confirmation_defaults_to_cancel_and_only_explicit_apply_emits_delete() {
    let (mut state, target) = state();
    state.begin_delete(target.clone(), 1);
    assert!(state.set_delete_review(1, review(target)));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Cancel")
            .is_focused()
    );
    let visible_labels: Vec<String> = harness
        .get_all_by_role(Role::Label)
        .filter_map(|node| {
            let node = node.accesskit_node();
            node.value().or_else(|| node.label())
        })
        .map(|text| text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    for expected in ["Record: 101", "Rows to delete: 1"] {
        assert!(
            visible_labels.iter().any(|label| label == expected),
            "missing {expected} from actual visible labels: {visible_labels:?}"
        );
    }
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(!harness.state().0.deletion_dialog_open());
    assert!(
        harness
            .state()
            .1
            .iter()
            .any(|action| matches!(action, Action::CloseDelete { request_id: 1 }))
    );
    assert!(
        !harness
            .state()
            .1
            .iter()
            .any(|action| matches!(action, Action::ApplyDelete { .. }))
    );

    let state = &mut harness.state_mut().0;
    let target = state.current_delete_target().unwrap();
    state.begin_delete(target.clone(), 2);
    assert!(state.set_delete_review(2, review(target)));
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!harness.state().0.deletion_dialog_open());
    assert!(
        harness
            .state()
            .1
            .iter()
            .any(|action| { matches!(action, Action::CloseDelete { request_id: 2 }) })
    );
    assert!(
        !harness
            .state()
            .1
            .iter()
            .any(|action| { matches!(action, Action::ApplyDelete { .. }) })
    );

    let state = &mut harness.state_mut().0;
    let target = state.current_delete_target().unwrap();
    state.begin_delete(target.clone(), 3);
    assert!(state.set_delete_review(3, review(target)));
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Delete reviewed row")
        .click_accesskit();
    harness.run();
    assert!(
        harness
            .state()
            .1
            .iter()
            .any(|action| matches!(action, Action::ApplyDelete { review_id: 30 }))
    );
}
