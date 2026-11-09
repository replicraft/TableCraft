//! UI-private source-free metadata setup shared by saved-field interaction tests.
use crate::{Column, Data, Database, ObjectKind, ObjectTarget, State, Table};

pub(in crate::workspace) fn saved_table(title: &str, columns: Vec<Column>) -> State {
    let mut state = State {
        database: Some(Database {
            document_revision: 7,
            title_path: None,
            title: title.into(),
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
    state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Table {
                name: "Table1".into(),
            },
        })
        .unwrap();
    assert!(state.set_table_data(Data {
        source_revision: Some(7),
        row_ordinals: None,
        table: "Table1".into(),
        offset: 0,
        columns,
        rows: Vec::new(),
        has_more: false,
    }));
    state
}
