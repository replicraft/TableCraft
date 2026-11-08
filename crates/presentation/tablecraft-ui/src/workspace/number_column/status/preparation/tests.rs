use crate::workspace::number_column::NumberColumnRequest;
use crate::{Data, Database, ObjectKind, ObjectTarget, State, Table};

fn reviewed() -> (State, NumberColumnRequest) {
    let mut state = State {
        number_column_available: true,
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
        source_revision: Some(7),
        row_ordinals: None,
        table: "Table1".into(),
        offset: 0,
        columns: vec!["ID".into()],
        rows: Vec::new(),
        has_more: false,
    }));
    assert!(state.begin_number_column_inspection(&tab, 80).is_ok());
    assert!(state.set_number_column_availability(&tab, 80, true));
    assert!(state.begin_number_column_name(&tab, "Score".into()));
    let request = state.number_column_review_request().unwrap();
    assert!(state.begin_number_column_review(&request, 81));
    assert!(state.accept_number_column_review(&request, 81, 90));
    (state, request)
}

#[test]
fn accepted_actor_binding_has_no_post_start_admission_or_allocated_payload() {
    let (mut state, request) = reviewed();
    let bytes = state.object_bytes().unwrap();
    let prepared = state.prepare_number_column_save(&request, 81, 90).unwrap();
    let identity = prepared.bind(100);
    assert_eq!(identity.request_id, 81);
    assert_eq!(identity.review_id, 90);
    assert_eq!(identity.save_id, 100);
    assert!(state.matches_number_column_save(&request, identity));
    assert_eq!(state.object_bytes(), Some(bytes));
    assert!(state.number_column_save_status().unwrap().1.is_none());
}

#[test]
fn refused_or_abandoned_start_preserves_the_exact_unsaved_name() {
    let (mut state, request) = reviewed();
    assert!(state.prepare_number_column_save(&request, 82, 90).is_none());
    drop(state.prepare_number_column_save(&request, 81, 90).unwrap());
    assert_eq!(state.number_column_review_request().unwrap(), request);
    assert!(state.number_column_save_status().is_none());
}
