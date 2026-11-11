use crate::workspace::field_addition::FieldAdditionRequest;
use crate::{Data, Database, ObjectKind, ObjectTarget, State, Table};
use tablecraft_builders::table::FieldAdditionDeclaration;

fn reviewed() -> (State, FieldAdditionRequest) {
    let mut state = State {
        field_addition_available: true,
        database: Some(Database {
            document_revision: 7,
            title_path: None,
            title: "Number binding".into(),
            format: "ACE 16".into(),
            queries: Vec::new(),
            tables: vec![Table {
                name: "Table1".into(),
                local: true,
                system: false,
                storage: "Local".into(),
            }],
        }),
        ..State::default()
    };
    state.dismiss_backstage();
    let tab = state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Table {
                name: "Table1".into(),
            },
        })
        .unwrap()
        .tab;
    assert!(state.set_table_data(Data {
        query_contents: None,
        source_revision: Some(7),
        row_ordinals: None,
        table: "Table1".into(),
        offset: 0,
        columns: vec!["ID".into()],
        rows: Vec::new(),
        has_more: false,
    }));
    assert!(state.begin_field_addition_inspection(&tab, 80).is_ok());
    assert!(state.set_field_addition_availability(&tab, 80, true, false, Default::default()));
    assert!(state.begin_field_addition_name(
        &tab,
        "Score".into(),
        FieldAdditionDeclaration::NumberLongZero
    ));
    let request = state.field_addition_review_request().unwrap();
    assert!(state.begin_field_addition_review(&request, 81));
    assert!(state.accept_field_addition_review(&request, 81, 90));
    (state, request)
}

#[test]
fn accepted_actor_binding_has_no_post_start_admission_or_allocated_payload() {
    let (mut state, request) = reviewed();
    let bytes = state.object_bytes().unwrap();
    let prepared = state.prepare_field_addition_save(&request, 81, 90).unwrap();
    let identity = prepared.bind(100);
    assert_eq!(identity.request_id, 81);
    assert_eq!(identity.review_id, 90);
    assert_eq!(identity.save_id, 100);
    assert!(state.matches_field_addition_save(&request, identity));
    assert_eq!(state.object_bytes(), Some(bytes));
    assert!(state.field_addition_save_status().unwrap().1.is_none());
}

#[test]
fn refused_or_abandoned_start_preserves_the_exact_unsaved_name() {
    let (mut state, request) = reviewed();
    assert!(
        state
            .prepare_field_addition_save(&request, 82, 90)
            .is_none()
    );
    drop(state.prepare_field_addition_save(&request, 81, 90).unwrap());
    assert_eq!(state.field_addition_review_request().unwrap(), request);
    assert!(state.field_addition_save_status().is_none());
}
