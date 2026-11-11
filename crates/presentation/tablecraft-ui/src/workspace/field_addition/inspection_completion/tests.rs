use crate::workspace::field_addition::tests::interaction::state;
use crate::{ObjectKind, ObjectTarget};
use tablecraft_builders::table::FieldAdditionKind;

#[test]
fn exact_parked_owner_receives_terminal_fact_without_touching_current_operation() {
    let mut state = state(true);
    let owner = state.active_object().unwrap();
    assert!(state.begin_field_addition_inspection(&owner, 80).is_ok());
    let scratch = state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    state.busy = true;
    state.error = Some("current operation".into());
    assert!(!state.complete_field_addition_inspection(
        80,
        8,
        "Table1",
        true,
        false,
        Default::default()
    ));
    assert!(!state.complete_field_addition_inspection(
        80,
        7,
        "Other",
        true,
        false,
        Default::default()
    ));
    assert!(state.has_pending_field_addition_inspection(80));
    assert!(state.complete_field_addition_inspection(
        80,
        7,
        "Table1",
        true,
        false,
        Default::default()
    ));
    assert!(!state.has_pending_field_addition_inspection(80));
    assert_eq!(state.active_object(), Some(scratch));
    assert!(state.busy);
    assert_eq!(state.error.as_deref(), Some("current operation"));
    assert!(!state.can_name_field_addition(&owner, FieldAdditionKind::Number));
    state.prune_field_addition_inspections();
    assert_eq!(
        state.field_addition_inspection(&owner).unwrap().1,
        Some(true)
    );
}

#[test]
fn closed_pending_owner_is_retired_only_after_exact_terminal_report_even_at_payload_ceiling() {
    let mut state = state(true);
    let owner = state.active_object().unwrap();
    assert!(state.begin_field_addition_inspection(&owner, 80).is_ok());
    assert!(state.close_object(&owner));
    state.prune_field_addition_inspections();
    assert!(state.has_pending_field_addition_inspection(80));
    state.sql = String::with_capacity(crate::workspace::objects::MAX_RETAINED_BYTES);
    state.error = Some("current operation".into());
    assert!(!state.fail_recorded_field_addition_inspection(79, "unrelated".into()));
    assert!(state.fail_recorded_field_addition_inspection(80, "cancelled read".into()));
    assert_eq!(state.field_addition_inspection(&owner).unwrap().2, Some(""));
    assert_eq!(state.error.as_deref(), Some("current operation"));
    state.prune_field_addition_inspections();
    assert!(state.field_addition_inspection(&owner).is_none());
}

#[test]
fn unissued_inspection_interrupts_only_its_exact_reservation_without_inventing_an_id() {
    let mut state = state(true);
    let owner = state.active_object().unwrap();
    assert!(state.prepare_field_addition_inspection(&owner).is_ok());
    let mut other = owner.clone();
    other.target.document_revision += 1;
    assert!(!state.interrupt_prepared_field_addition_inspection(&other));
    assert_eq!(
        state.field_addition_inspection(&owner),
        Some((true, None, None))
    );
    assert!(state.interrupt_prepared_field_addition_inspection(&owner));
    assert!(!state.interrupt_prepared_field_addition_inspection(&owner));
    assert_eq!(
        state.field_addition_inspection(&owner),
        Some((false, None, None))
    );
    assert!(state.field_addition_inspection_interrupted(&owner));
    assert!(!state.bind_field_addition_inspection(&owner, 80));
    assert!(!state.has_pending_field_addition_inspection(0));
    assert!(state.retry_field_addition_inspection(&owner));
    assert!(state.needs_field_addition_inspection(&owner));

    assert!(state.begin_field_addition_inspection(&owner, 81).is_ok());
    assert!(!state.interrupt_prepared_field_addition_inspection(&owner));
    assert!(state.has_pending_field_addition_inspection(81));
}
