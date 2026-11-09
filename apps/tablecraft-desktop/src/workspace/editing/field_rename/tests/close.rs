//! Deferred close composes actual local choices, native ACK and full witness read.
use super::{finish_schema, inspected};
use crate::workspace::Desktop;
use crate::workspace::test_support::{self, finish_reads};
use egui::{Id, accesskit::Role};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::SourceCase, saved_field_rename,
};
use tablecraft_ui::{ObjectKind, ObjectTarget};

#[test]
fn dirty_close_cancel_retains_name_then_save_closes_only_after_checked_publication() {
    let [before, after] = saved_field_rename::pair(SourceCase::Number);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    let table = harness.state().state.active_object().unwrap();
    let scratch = harness
        .state_mut()
        .state
        .prepare_object_open(ObjectTarget {
            document_revision: table.target.document_revision,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    let exact_scratch = " SELECT [Score] FROM [Table1]; -- retain exact scratch\n";
    assert!(
        harness
            .state_mut()
            .state
            .replace_query_sql(exact_scratch.into())
    );
    harness.state_mut().activate_object(table.clone());
    finish_reads(&mut harness);
    let target = inspected(&mut harness, "Score");
    assert!(
        harness
            .state_mut()
            .state
            .set_field_rename_name(&target, "Total".into())
    );

    harness.state_mut().close_object(table.clone());
    harness.run_steps(2);
    assert!(harness.state().state.field_rename_close_open());
    assert!(harness.state().deferred_draft_action.is_some());
    click_close_choice(&mut harness, "Cancel");
    harness.run_steps(2);
    assert!(!harness.state().state.field_rename_close_open());
    assert!(harness.state().deferred_draft_action.is_none());
    assert_eq!(
        harness.state().state.field_rename_proposal().unwrap().1,
        "Total"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
    assert_eq!(harness.state().state.active_object().unwrap(), table);

    harness.state_mut().close_object(table.clone());
    harness.run_steps(2);
    click_close_choice(&mut harness, "Save");
    harness.run_steps(1);
    assert!(harness.state().deferred_draft_action.is_some());
    assert!(harness.state().editing.field_rename.is_some());
    assert!(
        harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.id == table.id)
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "readonly review cannot close or write the table"
    );

    finish_schema(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
    assert!(harness.state().deferred_draft_action.is_none());
    assert!(!harness.state().state.field_rename_open());
    let tabs = harness.state().state.object_tabs();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].id, scratch.id);
    assert_eq!(tabs[0].target.kind, ObjectKind::Sql);
    assert_ne!(
        tabs[0].target.document_revision,
        scratch.target.document_revision
    );
    assert_eq!(harness.state().state.sql, exact_scratch);
}

fn click_close_choice(harness: &mut Harness<'_, Desktop>, label: &str) {
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new("field_rename_close")))
        .expect("actual rename close modal");
    let choices = harness
        .get_all_by_role_and_label(Role::Button, label)
        .filter(|button| modal.contains_rect(button.rect()))
        .collect::<Vec<_>>();
    assert_eq!(
        choices.len(),
        1,
        "exact choice in actual rename close modal"
    );
    assert!(!choices[0].accesskit_node().is_disabled());
    choices[0].click_accesskit();
}
