use super::*;
use crate::workspace::{ObjectKind, ObjectTarget};
use crate::{Data, Database, Table};
use tablecraft_builders::table::FieldAdditionDeclaration;

fn base_state() -> State {
    let mut state = State {
        database: Some(Database {
            title_path: None,
            document_revision: 7,
            title: "Example".into(),
            format: "ACE16".into(),
            tables: ["A", "B"]
                .into_iter()
                .map(|name| Table {
                    name: name.into(),
                    local: true,
                    system: false,
                    storage: "Local".into(),
                })
                .collect(),
            queries: vec![],
        }),
        field_addition_available: true,
        ..State::default()
    };
    state.dismiss_backstage();
    state
}

fn begin_name(state: &mut State, tab: &ObjectTab) {
    assert!(state.begin_field_addition_inspection(tab, 1).is_ok());
    assert!(state.set_field_addition_availability(tab, 1, true, false, Default::default()));
    assert!(state.begin_field_addition_name(
        tab,
        "Field1".into(),
        FieldAdditionDeclaration::NumberLongZero
    ));
}

fn naming() -> (State, ObjectTab) {
    let mut state = base_state();
    let tab = table(&mut state, "A");
    begin_name(&mut state, &tab);
    (state, tab)
}

fn table(state: &mut State, name: &str) -> ObjectTab {
    let tab = state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Table { name: name.into() },
        })
        .unwrap()
        .tab;
    assert!(state.set_table_data(Data {
        source_revision: Some(7),
        row_ordinals: None,
        table: name.into(),
        offset: 0,
        columns: vec!["ID".into()],
        rows: vec![],
        has_more: false,
    }));
    tab
}

fn pointer(state: &State) -> *const u8 {
    state.field_addition.draft.as_ref().unwrap().column.as_ptr()
}

#[test]
fn exact_owner_close_prompts_and_cancel_keeps_name_revision_and_object() {
    let (mut state, tab) = naming();
    let before_pointer = pointer(&state);
    assert!(state.can_close_object(&tab));
    assert!(!state.close_object(&tab));
    assert!(state.field_addition_close_open());
    assert!(!state.can_edit_field_addition_name(&tab));
    let result = state
        .choose_field_addition_close(FieldAdditionCloseDecision::Cancel)
        .unwrap();
    assert_eq!(result.tab, tab);
    assert_eq!(result.draft_revision, 0);
    assert_eq!(result.decision, FieldAdditionCloseDecision::Cancel);
    assert!(result.request.is_none());
    assert_eq!(state.field_addition_name().unwrap().1, "Field1");
    assert_eq!(pointer(&state), before_pointer);
    assert!(state.objects.contains_tab(&tab));
    assert!(!state.field_addition_close_open());
}

#[test]
fn save_returns_exact_request_without_persisting_or_clearing_the_name() {
    let (mut state, tab) = naming();
    assert!(state.set_field_addition_name(&tab, " Score exact ".into()));
    let revision = state.field_addition_name().unwrap().2;
    let before_pointer = pointer(&state);
    assert!(state.request_field_addition_close(&tab));
    assert!(state.can_save_field_addition_close());
    let result = state
        .choose_field_addition_close(FieldAdditionCloseDecision::Save)
        .unwrap();
    assert_eq!(result.decision, FieldAdditionCloseDecision::Save);
    assert_eq!(result.draft_revision, revision);
    let request = result.request.unwrap();
    assert_eq!(request.tab, tab);
    assert_eq!(request.draft_revision, revision);
    assert_eq!(request.column, " Score exact ");
    assert!(state.field_addition_request_is_current(&request));
    assert_eq!(pointer(&state), before_pointer);
    assert!(!state.field_addition_close_open());
    assert!(!state.close_object_confirmed(&tab));
}

#[test]
fn explicit_discard_only_removes_the_matching_unstarted_name() {
    let (mut state, tab) = naming();
    assert!(!state.close_object_confirmed(&tab));
    assert!(state.request_field_addition_close(&tab));
    let result = state
        .choose_field_addition_close(FieldAdditionCloseDecision::Discard)
        .unwrap();
    assert_eq!(result.decision, FieldAdditionCloseDecision::Discard);
    assert!(result.request.is_none());
    assert!(!state.field_addition_open());
    assert!(state.objects.contains_tab(&tab));
    assert!(state.close_object_confirmed(&tab));
}

#[test]
fn stale_choices_keep_newer_text_and_cannot_discard_a_started_review() {
    let (mut state, tab) = naming();
    assert!(state.request_field_addition_close(&tab));
    assert!(state.field_addition.replace_name(&tab, "Newer".into()));
    assert!(
        state
            .choose_field_addition_close(FieldAdditionCloseDecision::Save)
            .is_none()
    );
    assert!(
        state
            .choose_field_addition_close(FieldAdditionCloseDecision::Discard)
            .is_none()
    );
    assert!(state.field_addition_close_open());
    assert_eq!(state.field_addition_name().unwrap().1, "Newer");
    state
        .choose_field_addition_close(FieldAdditionCloseDecision::Cancel)
        .unwrap();
    assert!(state.request_field_addition_close(&tab));
    let request = state.field_addition.request(&tab).unwrap();
    assert!(state.field_addition.begin_review(&request, 50));
    assert!(
        state
            .choose_field_addition_close(FieldAdditionCloseDecision::Discard)
            .is_none()
    );
    assert_eq!(state.field_addition_name().unwrap().1, "Newer");
    assert!(
        state
            .choose_field_addition_close(FieldAdditionCloseDecision::Cancel)
            .is_some()
    );
    assert!(
        state
            .field_addition
            .matches(&request, Phase::Reviewing { request_id: 50 })
    );
}

#[test]
fn parked_owner_is_activated_only_after_save_choice_without_a_global_bypass() {
    let mut state = base_state();
    let other = table(&mut state, "B");
    let owner = table(&mut state, "A");
    begin_name(&mut state, &owner);
    // Normal navigation is blocked by this unstarted name; the admitted
    // mechanics create the retained parked-owner scenario without removing it.
    assert!(state.objects.contains_tab(&other));
    assert!(state.activate_object_after_admission(&other));
    let before_pointer = pointer(&state);
    assert!(state.request_field_addition_close(&owner));
    assert_eq!(state.active_object().unwrap(), other);
    assert!(state.can_save_field_addition_close());
    let result = state
        .choose_field_addition_close(FieldAdditionCloseDecision::Save)
        .unwrap();
    assert_eq!(state.active_object().unwrap(), owner);
    assert_eq!(result.request.unwrap().tab, owner);
    assert_eq!(pointer(&state), before_pointer);
    assert_eq!(state.data.as_ref().unwrap().table, "A");
}

#[test]
fn readonly_or_empty_name_still_allows_discard_but_never_save() {
    let (mut state, tab) = naming();
    assert!(state.set_field_addition_name(&tab, String::new()));
    state.field_addition_available = false;
    assert!(state.request_field_addition_close(&tab));
    assert!(!state.can_save_field_addition_close());
    assert!(
        state
            .choose_field_addition_close(FieldAdditionCloseDecision::Save)
            .is_none()
    );
    assert!(
        state
            .choose_field_addition_close(FieldAdditionCloseDecision::Discard)
            .is_some()
    );
}
