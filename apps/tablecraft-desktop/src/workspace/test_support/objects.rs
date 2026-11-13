//! App-private scratch-tab widgets shared by schema preservation acceptance.
use super::finish_reads;
use crate::workspace::Desktop;
use egui::{Id, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_ui::{ObjectKind, ObjectTab, ObjectTarget};

pub(in crate::workspace) const EXACT_SCRATCH: &str =
    " SELECT [Score], [Label] FROM [Table1]; -- keep raw scratch\n";

pub(in crate::workspace) fn scratch(
    harness: &mut Harness<'_, Desktop>,
    table: &ObjectTab,
) -> ObjectTab {
    let scratch = harness
        .state_mut()
        .state
        .prepare_object_open(ObjectTarget {
            document_revision: table.target.document_revision,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert!(
        harness
            .state_mut()
            .state
            .replace_query_sql(EXACT_SCRATCH.into())
    );
    harness.state_mut().activate_object(table.clone());
    finish_reads(harness);
    scratch
}

/// Consume the actual scratch/Design disposal modal already issued by a global
/// Open. Native read completion remains the caller's existing bounded wait.
pub(in crate::workspace) fn discard_query_close_for_open(harness: &mut Harness<'_, Desktop>) {
    assert!(harness.state().state.query_design_close_open());
    assert!(harness.state().deferred_draft_action.is_some());
    harness.run_steps(2);
    let label = harness
        .state()
        .state
        .localizer
        .text("saved-query-close-discard");
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new("query_design_close")))
        .expect("actual query close modal before the global Open");
    let choices = harness
        .get_all_by_role_and_label(Role::Button, &label)
        .filter(|button| modal.contains_rect(button.rect()))
        .collect::<Vec<_>>();
    assert_eq!(
        choices.len(),
        1,
        "exact Discard in the actual query-close modal"
    );
    assert!(!choices[0].accesskit_node().is_disabled());
    choices[0].click_accesskit();
    drop(choices);
    harness.run_steps(2);
    assert!(!harness.state().state.query_design_close_open());
    assert!(harness.state().deferred_draft_action.is_none());
}
