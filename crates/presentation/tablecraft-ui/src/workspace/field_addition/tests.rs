use super::{FieldAdditionRequest, Model, Phase};
use crate::workspace::{ObjectKind, ObjectTab, ObjectTarget};
use std::num::NonZeroU8;
use tablecraft_builders::table::{FieldAdditionDeclaration, FieldAdditionKind};

fn tab(id: u64, revision: u64, name: &str) -> ObjectTab {
    ObjectTab {
        id,
        target: ObjectTarget {
            document_revision: revision,
            kind: ObjectKind::Table { name: name.into() },
        },
    }
}

fn ready() -> (Model, ObjectTab) {
    let tab = tab(4, 19, "Table1");
    let mut model = Model::default();
    assert!(model.inspect(tab.clone(), 80).is_ok());
    assert!(model.observed(&tab, 80, true, false, Default::default()));
    assert!(model.begin(
        &tab,
        "Field1".into(),
        FieldAdditionDeclaration::NumberLongZero
    ));
    (model, tab)
}

#[test]
fn only_positive_current_source_observation_permits_naming() {
    let mut model = Model::default();
    let owner = tab(4, 19, "Table1");
    assert!(!model.available(&owner, FieldAdditionKind::Number));
    assert!(model.inspect(owner.clone(), 80).is_ok());
    assert!(!model.available(&owner, FieldAdditionKind::Number));
    assert!(!model.observed(&owner, 79, true, false, Default::default()));
    assert!(!model.observed(&tab(5, 19, "Table1"), 80, true, false, Default::default()));
    assert!(!model.observed(&tab(4, 20, "Table1"), 80, true, false, Default::default()));
    assert!(!model.observed(&tab(4, 19, "table1"), 80, true, false, Default::default()));
    assert!(model.observed(&owner, 80, false, false, Default::default()));
    assert!(!model.begin(
        &owner,
        "Score".into(),
        FieldAdditionDeclaration::NumberLongZero
    ));
    assert!(!model.observed(&owner, 80, true, false, Default::default()));
    assert!(model.retry_inspection(&owner));
    assert!(model.inspect(owner.clone(), 81).is_ok());
    assert!(model.observed(&owner, 81, true, false, Default::default()));
    assert!(model.begin(
        &owner,
        "Score".into(),
        FieldAdditionDeclaration::NumberLongZero
    ));
    assert!(!model.available(&owner, FieldAdditionKind::Number));
}

#[test]
fn exact_last_header_text_and_revision_are_captured_after_widgets() {
    let (mut model, owner) = ready();
    let before = model.request(&owner).unwrap();
    assert!(model.replace_name(&owner, " Score ".into()));
    assert!(!model.begin_review(&before, 81));
    let after = model.request(&owner).unwrap();
    assert_eq!(after.column, " Score ");
    assert_eq!(after.draft_revision, 1);
    assert!(model.replace_name(&owner, " Score ".into()));
    assert_eq!(model.request(&owner).unwrap(), after);
    assert!(model.begin_review(&after, 81));
    assert!(!model.replace_name(&owner, "Other".into()));
    assert!(!model.discard(&owner, 1));
}

#[test]
fn bounded_header_edit_refusal_preserves_name_and_revision() {
    let (mut model, owner) = ready();
    let original = model.request(&owner).unwrap();
    for name in ["a".repeat(65), "😀".repeat(33)] {
        assert!(!model.replace_name(&owner, name));
        assert_eq!(model.request(&owner).unwrap(), original);
    }
    assert!(model.replace_name(&owner, String::new()));
    assert!(model.request(&owner).is_none());
    assert!(model.replace_name(&owner, "Score".into()));
    assert_eq!(model.request(&owner).unwrap().draft_revision, 2);
}

#[test]
fn actual_review_and_save_ids_bind_the_exact_original_naming_request() {
    let (mut model, owner) = ready();
    let request = model.request(&owner).unwrap();
    assert!(model.begin_review(&request, 81));
    assert!(!model.reviewed(&request, 82, 90));
    let changed = FieldAdditionRequest {
        declaration: FieldAdditionDeclaration::NumberLongZero,
        tab: owner.clone(),
        draft_revision: 0,
        column: "Other".into(),
    };
    assert!(!model.reviewed(&changed, 81, 90));
    assert!(model.reviewed(&request, 81, 90));
    assert!(!model.saving(&request, 81, 91, 100));
    let retained = model.retained_bytes().unwrap();
    assert!(model.saving(&request, 81, 90, 100));
    assert_eq!(model.retained_bytes(), Some(retained));
    assert_eq!(
        model.draft().unwrap().3,
        Phase::Saving {
            request_id: 81,
            review_id: 90,
            save_id: 100,
        }
    );
    assert!(!model.release_review(&request, 81));
    assert!(!model.discard(&owner, 0));
    assert!(model.inspect(tab(4, 20, "Table1"), 82).is_err());
}

#[test]
fn refused_review_preserves_exact_name_and_late_callbacks_lose_authority() {
    let (mut model, owner) = ready();
    let request = model.request(&owner).unwrap();
    assert!(model.begin_review(&request, 81));
    assert!(!model.release_review(&request, 82));
    assert!(model.release_review(&request, 81));
    assert_eq!(model.request(&owner).unwrap(), request);
    assert!(model.replace_name(&owner, "Score".into()));
    assert!(!model.reviewed(&request, 81, 90));
    assert!(!model.discard(&owner, 0));
    assert!(model.discard(&owner, 1));
    assert!(model.draft().is_none());
}

#[test]
fn inspection_and_draft_account_for_owned_capacity_without_touching_data() {
    let (model, owner) = ready();
    let request = model.request(&owner).unwrap();
    let request_bytes = request.retained_bytes().unwrap();
    assert!(request_bytes >= size_of::<FieldAdditionRequest>() + "Table1".len() + "Field1".len());
    assert!(
        model.retained_bytes().unwrap() >= size_of::<Model>() + 2 * "Table1".len() + "Field1".len()
    );
    let mut model = Model::default();
    let sql = ObjectTab {
        id: 4,
        target: ObjectTarget {
            document_revision: 19,
            kind: ObjectKind::Sql,
        },
    };
    assert!(model.inspect(sql, 80).is_err());
    assert_eq!(model.retained_bytes(), Some(size_of::<Model>()));
}

fn saving_state() -> (
    crate::State,
    FieldAdditionRequest,
    super::FieldAdditionSaveIdentity,
) {
    let (mut model, owner) = ready();
    let request = model.request(&owner).unwrap();
    assert!(model.begin_review(&request, 81));
    assert!(model.reviewed(&request, 81, 90));
    assert!(model.saving(&request, 81, 90, 100));
    let mut state = crate::State {
        field_addition: model,
        ..crate::State::default()
    };
    let identity = super::FieldAdditionSaveIdentity {
        request_id: 81,
        review_id: 90,
        save_id: 100,
    };
    assert!(state.begin_field_addition_save_status(identity));
    (state, request, identity)
}

#[test]
fn recovery_changes_only_the_actual_actor_id_and_rejects_old_controls() {
    use crate::EditSaveOutcome;
    let (mut state, request, identity) = saving_state();
    assert!(state.set_field_addition_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message: "pending journal".into(),
            can_recover: true,
            can_release: false,
        }
    ));
    assert!(!state.can_resolve_field_addition_save(identity, false));
    assert!(!state.can_acknowledge_field_addition_save(identity));
    let next = state
        .begin_field_addition_resolution(&request, identity, 101, true)
        .unwrap();
    assert!(!state.cancel_field_addition_operation(identity));
    assert!(
        !state.set_field_addition_save_outcome(identity, EditSaveOutcome::Failed("late".into()))
    );
    assert!(state.matches_field_addition_save(&request, next));
    assert!(state.cancel_field_addition_operation(next));
    assert!(!state.cancel_field_addition_operation(next));
}

#[test]
fn report_admission_refusal_retains_terminal_permissions_and_reopen_retry() {
    use crate::EditSaveOutcome;
    let (mut state, request, identity) = saving_state();
    state.sql = String::with_capacity(crate::workspace::objects::MAX_RETAINED_BYTES);
    assert!(state.set_field_addition_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message: "retained".into(),
            can_recover: true,
            can_release: true,
        }
    ));
    assert!(state.can_resolve_field_addition_save(identity, true));
    assert!(state.can_resolve_field_addition_save(identity, false));
    let next = state
        .begin_field_addition_resolution(&request, identity, 101, true)
        .unwrap();
    assert!(
        state.set_field_addition_save_outcome(next, EditSaveOutcome::Saved("known AFTER".into()))
    );
    assert!(state.can_acknowledge_field_addition_save(next));
    assert!(state.begin_field_addition_reopen(next));
    assert!(state.cancel_field_addition_operation(next));
    assert!(state.fail_field_addition_reopen(next, "cancelled read".into()));
    assert!(state.can_retry_field_addition_reopen(next));
    assert!(
        matches!(state.field_addition_save_status(), Some((_, Some(EditSaveOutcome::Saved(message)))) if message.is_empty())
    );
    assert!(state.field_addition_status.as_ref().unwrap().omitted);
    assert!(state.matches_field_addition_save(&request, next));
}

#[test]
fn source_observation_is_cached_across_tab_activation_and_errors_require_explicit_retry() {
    let mut model = Model::default();
    let first = tab(4, 19, "Table1");
    let second = tab(5, 19, "Items");
    assert!(model.inspect(first.clone(), 80).is_ok());
    assert!(model.inspect(first.clone(), 81).is_err());
    assert!(!model.retry_inspection(&first));
    assert!(model.observed(&first, 80, true, false, Default::default()));
    assert!(model.inspect(second.clone(), 81).is_ok());
    assert!(model.inspection_failed(&second, 81, "source check refused".into()));
    assert!(model.inspect(second.clone(), 82).is_err());
    assert!(!model.needs_inspection(&first));
    assert!(!model.needs_inspection(&second));
    assert!(model.available(&first, FieldAdditionKind::Number));
    assert!(!model.available(&second, FieldAdditionKind::Number));
    assert!(model.retry_inspection(&second));
    assert!(model.inspect(second.clone(), 82).is_ok());
    assert!(!model.observed(&second, 81, true, false, Default::default()));
    assert!(model.observed(&second, 82, true, false, Default::default()));
}

#[test]
fn inspection_capacity_keeps_existing_terminal_and_pending_owners() {
    let mut model = Model::default();
    for id in 0..crate::workspace::objects::MAX_OBJECTS as u64 {
        assert!(
            model
                .inspect(tab(id, 19, &format!("Table{id}")), id)
                .is_ok()
        );
    }
    let first = tab(0, 19, "Table0");
    assert!(model.observed(&first, 0, true, false, Default::default()));
    assert_eq!(
        model.inspect(tab(99, 19, "More"), 99),
        Err(super::FieldAdditionInspectionError::Capacity)
    );
    assert!(model.available(&first, FieldAdditionKind::Number));
    let last_id = crate::workspace::objects::MAX_OBJECTS as u64 - 1;
    assert!(model.observed(
        &tab(last_id, 19, &format!("Table{last_id}")),
        last_id,
        true,
        false,
        Default::default()
    ));
}

pub(in crate::workspace::field_addition) mod interaction;

#[test]
fn closed_pending_owner_resolves_before_retirement_and_sequential_tab_cycles_do_not_exhaust_cache()
{
    let mut model = Model::default();
    let closed = tab(4, 19, "Table1");
    assert!(model.inspect(closed.clone(), 80).is_ok());
    model.prune_inspections(|_| false);
    assert!(!model.needs_inspection(&closed));
    assert!(model.inspection_failed(&closed, 80, "cancelled read".into()));
    model.prune_inspections(|_| false);
    assert!(model.needs_inspection(&closed));
    for id in 0..(crate::workspace::objects::MAX_OBJECTS as u64 * 3) {
        let owner = tab(id + 100, 19, "Table1");
        assert!(model.inspect(owner.clone(), id + 100).is_ok());
        assert!(model.observed(&owner, id + 100, true, false, Default::default()));
        model.prune_inspections(|_| false);
        assert!(model.needs_inspection(&owner));
    }
}

#[test]
fn one_observation_preserves_independent_gallery_facts_and_full_declaration_identity() {
    let owner = tab(4, 19, "Table1");
    let mut model = Model::default();
    assert!(model.inspect(owner.clone(), 80).is_ok());
    assert!(model.observed(&owner, 80, false, true, Default::default()));
    assert!(!model.available(&owner, FieldAdditionKind::Number));
    assert!(model.available(&owner, FieldAdditionKind::ShortText));
    assert!(!model.begin(
        &owner,
        "Label".into(),
        FieldAdditionDeclaration::NumberLongZero
    ));
    let text = FieldAdditionDeclaration::ui_default(FieldAdditionKind::ShortText);
    assert!(model.begin(&owner, "Label".into(), text));
    let request = model.request(&owner).unwrap();
    assert_eq!(request.declaration, text);
    let mut changed = model.request(&owner).unwrap();
    changed.declaration = FieldAdditionDeclaration::ShortText {
        max_utf16_units: NonZeroU8::new(80).unwrap(),
        unicode_compression: true,
    };
    assert!(!model.begin_review(&changed, 81));
    changed.declaration = FieldAdditionDeclaration::ShortText {
        max_utf16_units: NonZeroU8::new(255).unwrap(),
        unicode_compression: false,
    };
    assert!(!model.begin_review(&changed, 81));
    changed.declaration = FieldAdditionDeclaration::NumberLongZero;
    assert!(!model.begin_review(&changed, 81));
    assert_eq!(model.request(&owner).unwrap(), request);
    assert!(model.begin_review(&request, 81));
    assert!(!model.reviewed(&changed, 81, 90));
    assert!(model.reviewed(&request, 81, 90));
    assert!(!model.saving(&changed, 81, 90, 100));
    assert!(model.saving(&request, 81, 90, 100));
}
