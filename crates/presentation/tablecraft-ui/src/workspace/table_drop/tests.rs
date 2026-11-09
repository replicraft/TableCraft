//! Headless intent and lifecycle checks; these do not qualify native DROP support.
use super::{
    TableDropRequest, TableDropSaveIdentity,
    model::{Observation, Phase},
};
use crate::workspace::navigation::tests::harness;
use crate::{
    Action, Database, EditSaveOutcome, ObjectKind, ObjectTarget, SavedQuery, State, Table,
};
use egui::{Key, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use std::sync::Arc;

fn copy_catalog(database: &Database) -> Database {
    Database {
        document_revision: database.document_revision,
        title_path: database.title_path.clone(),
        title: database.title.clone(),
        format: database.format.clone(),
        tables: database
            .tables
            .iter()
            .map(|table| Table {
                name: table.name.clone(),
                local: table.local,
                system: table.system,
                storage: table.storage.clone(),
            })
            .collect(),
        queries: database
            .queries
            .iter()
            .map(|query| SavedQuery {
                object_id: query.object_id,
                name: query.name.clone(),
                system: query.system,
            })
            .collect(),
    }
}

fn target() -> ObjectTarget {
    ObjectTarget {
        document_revision: 17,
        kind: ObjectKind::Table {
            name: "First".into(),
        },
    }
}

fn observed() -> (State, Arc<TableDropRequest>) {
    let mut harness = harness();
    harness.state_mut().0.table_drop_available = true;
    assert!(harness.state_mut().0.begin_table_drop(target()));
    harness.run_steps(2);
    let request = harness
        .state()
        .1
        .iter()
        .find_map(|action| match action {
            Action::InspectTableDrop { request } => Some(Arc::clone(request)),
            _ => None,
        })
        .unwrap();
    assert!(
        harness
            .state_mut()
            .0
            .begin_table_drop_inspection(&request, 80)
    );
    assert!(
        harness
            .state_mut()
            .0
            .set_table_drop_source(&request, 80, Some(2))
    );
    let (state, _) = std::mem::take(harness.state_mut());
    (state, request)
}

fn saving() -> (State, Arc<TableDropRequest>, TableDropSaveIdentity) {
    let (mut state, request) = observed();
    state.table_drop.session.as_mut().unwrap().phase = Phase::Reviewing { read_id: None };
    assert!(state.begin_table_drop_review(&request, 81));
    let identity = TableDropSaveIdentity {
        request_id: 81,
        review_id: 90,
        save_id: 100,
    };
    assert!(state.begin_table_drop_save(&request, identity));
    (state, request, identity)
}

#[test]
fn focused_delete_inspects_before_yes_then_enter_issues_one_immediate_save_intent() {
    let mut harness = harness();
    harness.state_mut().0.table_drop_available = true;
    harness.state_mut().0.database.as_mut().unwrap().tables[0].name = "Items".into();
    harness.run_steps(2);
    harness.get_by_role_and_label(Role::Button, "Items").focus();
    harness.run_steps(1);
    harness.key_press(Key::Delete);
    harness.run_steps(2);
    let [Action::InspectTableDrop { request }] = harness.state().1.as_slice() else {
        panic!("Delete inspects the closed table before enabling confirmation");
    };
    let request = Arc::clone(request);
    assert_eq!(
        request.target,
        ObjectTarget {
            document_revision: 17,
            kind: ObjectKind::Table {
                name: "Items".into()
            },
        }
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Yes")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .state_mut()
            .0
            .begin_table_drop_inspection(&request, 80)
    );
    assert!(
        harness
            .state_mut()
            .0
            .set_table_drop_source(&request, 80, Some(2))
    );
    harness.run_steps(2);
    let visible_labels = harness
        .get_all_by_role(Role::Label)
        .filter_map(|node| {
            let node = node.accesskit_node();
            node.value().or_else(|| node.label())
        })
        .map(|text| text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect::<Vec<_>>();
    assert!(
        visible_labels
            .iter()
            .any(|label| label == "Declared records: 2"),
        "actual visible labels retain the declared count: {visible_labels:?}"
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Yes")
            .is_focused()
    );
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert_eq!(harness.state().1.iter().filter(|action| matches!(action, Action::DeleteTable { request: actual } if Arc::ptr_eq(actual, &request))).count(), 1);
    assert!(harness.state().0.object_tabs().is_empty());
    assert_eq!(harness.state().0.database.as_ref().unwrap().tables.len(), 2);
}

#[test]
fn unsupported_host_stale_system_linked_and_open_native_objects_refuse() {
    let mut harness = harness();
    assert!(!harness.state_mut().0.begin_table_drop(target()));
    harness.state_mut().0.table_drop_available = true;
    let mut stale = target();
    stale.document_revision += 1;
    assert!(!harness.state_mut().0.begin_table_drop(stale));
    harness.state_mut().0.database.as_mut().unwrap().tables[0].system = true;
    assert!(!harness.state_mut().0.begin_table_drop(target()));
    harness.state_mut().0.database.as_mut().unwrap().tables[0].system = false;
    harness.state_mut().0.database.as_mut().unwrap().tables[0].local = false;
    assert!(!harness.state_mut().0.begin_table_drop(target()));
    harness.state_mut().0.database.as_mut().unwrap().tables[0].local = true;
    assert!(
        harness
            .state_mut()
            .0
            .prepare_object_open(target())
            .is_some()
    );
    assert!(!harness.state_mut().0.begin_table_drop(target()));
}

#[test]
fn escape_dismisses_confirmation_and_cancels_only_the_actual_pending_read() {
    let mut harness = harness();
    harness.state_mut().0.table_drop_available = true;
    assert!(harness.state_mut().0.begin_table_drop(target()));
    harness.run_steps(2);
    let request = match &harness.state().1[0] {
        Action::InspectTableDrop { request } => Arc::clone(request),
        _ => panic!("inspection expected"),
    };
    assert!(
        harness
            .state_mut()
            .0
            .begin_table_drop_inspection(&request, 80)
    );
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().0.table_drop_open());
    assert!(
        harness
            .state()
            .1
            .iter()
            .any(|action| matches!(action, Action::CancelTableDropInspection { request_id: 80 }))
    );
    assert!(
        !harness
            .state()
            .1
            .iter()
            .any(|action| matches!(action, Action::DeleteTable { .. }))
    );
    assert!(
        !harness
            .state_mut()
            .0
            .set_table_drop_source(&request, 80, Some(2))
    );
}

#[test]
fn interrupted_inspection_retry_rejects_old_result_and_never_invents_read_ids() {
    let (mut state, request) = observed();
    state.table_drop.session.as_mut().unwrap().observation = Observation::Unrequested;
    let mut actions = Vec::new();
    state.collect_table_drop_inspection(&mut actions);
    assert!(!state.begin_table_drop_inspection(&request, 0));
    assert!(state.begin_table_drop_inspection(&request, 82));
    assert!(!state.set_table_drop_source(&request, 80, Some(2)));
    assert!(state.interrupt_table_drop_inspection(82));
    assert!(!state.set_table_drop_source(&request, 82, Some(2)));
    assert!(!state.begin_table_drop_review(&request, 83));
}

#[test]
fn unknown_actor_result_stays_visible_and_revokes_mutation_after_acknowledgement() {
    let (mut state, request, identity) = saving();
    let wrong = TableDropSaveIdentity {
        save_id: 101,
        ..identity
    };
    assert!(!state.set_table_drop_actor_lost(wrong, "unknown".into()));
    assert!(
        state.set_table_drop_actor_lost(identity, "No native terminal result was received".into())
    );
    assert!(state.native_source_stale());
    assert!(!state.begin_table_drop_reopen(identity));
    assert!(!state.can_resolve_table_drop_save(identity, true));
    assert!(state.acknowledge_uncertain_table_drop_save(identity));
    assert!(!state.table_drop_open());
    assert!(state.native_mutation_blocked());
    assert!(state.matches_table_drop_save(&request, identity));
}

#[test]
fn exact_reopen_removes_only_target_and_allows_empty_catalog() {
    let (mut state, request, identity) = saving();
    assert!(
        state.set_table_drop_save_outcome(identity, EditSaveOutcome::Saved("Known AFTER".into()))
    );
    assert!(state.begin_table_drop_reopen(identity));
    let mut fresh = copy_catalog(state.database.as_ref().unwrap());
    fresh.document_revision += 1;
    assert!(!state.resolve_table_drop_after_reopen(&request, identity, copy_catalog(&fresh)));
    fresh.tables.remove(0);
    let mut wrong = copy_catalog(&fresh);
    wrong.tables[0].storage = "Changed unexpectedly".into();
    assert!(!state.resolve_table_drop_after_reopen(&request, identity, wrong));
    assert!(state.resolve_table_drop_after_reopen(&request, identity, fresh));
    assert_eq!(state.database.as_ref().unwrap().tables[0].name, "Second");
    assert!(!state.table_drop_open());
    assert!(!state.native_source_stale());

    let (mut state, request, identity) = saving();
    state.database.as_mut().unwrap().tables.truncate(1);
    assert!(
        state.set_table_drop_save_outcome(identity, EditSaveOutcome::Saved("Known AFTER".into()))
    );
    assert!(state.begin_table_drop_reopen(identity));
    let mut fresh = copy_catalog(state.database.as_ref().unwrap());
    fresh.document_revision += 1;
    fresh.tables.clear();
    assert!(state.resolve_table_drop_after_reopen(&request, identity, fresh));
    assert!(state.database.as_ref().unwrap().tables.is_empty());
}

#[test]
fn source_free_sql_draft_and_tab_identity_survive_exact_drop_publication() {
    let mut harness = harness();
    let scratch = harness
        .state_mut()
        .0
        .prepare_object_open(ObjectTarget {
            document_revision: 17,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert!(
        harness
            .state_mut()
            .0
            .replace_query_sql(" SELECT * FROM [First];\n".into())
    );
    harness.state_mut().0.table_drop_available = true;
    assert!(harness.state_mut().0.begin_table_drop(target()));
    harness.run_steps(2);
    let request = match &harness.state().1[0] {
        Action::InspectTableDrop { request } => Arc::clone(request),
        _ => panic!("inspection expected"),
    };
    let state = &mut harness.state_mut().0;
    assert!(state.begin_table_drop_inspection(&request, 80));
    assert!(state.set_table_drop_source(&request, 80, Some(2)));
    state.table_drop.session.as_mut().unwrap().phase = Phase::Reviewing { read_id: None };
    assert!(state.begin_table_drop_review(&request, 81));
    let identity = TableDropSaveIdentity {
        request_id: 81,
        review_id: 90,
        save_id: 100,
    };
    assert!(state.begin_table_drop_save(&request, identity));
    assert!(
        state.set_table_drop_save_outcome(identity, EditSaveOutcome::Saved("Known AFTER".into()))
    );
    assert!(state.begin_table_drop_reopen(identity));
    let old_sql_revision = state.query_sql_revision;
    let mut fresh = copy_catalog(state.database.as_ref().unwrap());
    fresh.document_revision += 1;
    fresh.tables.remove(0);
    assert!(state.resolve_table_drop_after_reopen(&request, identity, fresh));
    let active = state.active_object().unwrap();
    assert_eq!(active.id, scratch.id);
    assert_eq!(active.target.document_revision, 18);
    assert_eq!(active.target.kind, ObjectKind::Sql);
    assert_eq!(state.sql, " SELECT * FROM [First];\n");
    assert_eq!(state.query_sql_revision, old_sql_revision + 1);
    assert!(state.data.is_none());
    assert!(state.schema.is_none());
}

#[test]
fn retained_permissions_and_failed_reopen_never_replay_a_confirmed_deletion() {
    let (mut state, request, identity) = saving();
    assert!(state.set_table_drop_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message: "Inspect retained native session".into(),
            can_recover: true,
            can_release: false,
        }
    ));
    assert!(!state.begin_table_drop_resolution(identity, false));
    assert!(state.begin_table_drop_resolution(identity, true));
    assert!(!state.begin_table_drop_resolution(identity, true));
    assert!(state.cancel_table_drop_operation(identity));
    assert!(!state.cancel_table_drop_operation(identity));
    assert!(
        state.set_table_drop_save_outcome(identity, EditSaveOutcome::Saved("Known AFTER".into()))
    );
    assert!(state.begin_table_drop_reopen(identity));
    assert!(state.fail_table_drop_reopen(identity, "Full witness read failed".into()));
    assert!(state.can_retry_table_drop_reopen(identity));
    assert!(state.return_from_table_drop_reopen(identity));
    assert!(!state.table_drop_open());
    assert!(state.native_mutation_blocked());
    assert!(!state.can_begin_table_drop(&request.target));
    assert!(state.begin_table_drop_reopen(identity));
    assert!(state.table_drop_reopening(identity));
}
