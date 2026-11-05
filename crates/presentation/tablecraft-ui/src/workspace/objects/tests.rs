use super::*;
use crate::{Cell, Data, Database, Table};

fn table(name: &str) -> ObjectTarget {
    ObjectTarget {
        document_revision: 7,
        kind: ObjectKind::Table { name: name.into() },
    }
}

fn state() -> State {
    let mut state = State {
        database: Some(Database {
            document_revision: 7,
            title_path: None,
            title: "Objects".into(),
            format: "Ace2016".into(),
            tables: ["A", "B"]
                .into_iter()
                .map(|name| Table {
                    name: name.into(),
                    system: false,
                    storage: "Local".into(),
                })
                .collect(),
            queries: vec![crate::SavedQuery {
                object_id: 9,
                name: "Q".into(),
                system: false,
            }],
        }),
        ..State::default()
    };
    state.dismiss_backstage();
    state
}

fn data(name: &str, value: &str) -> Data {
    Data {
        source_revision: Some(7),
        row_ordinals: None,
        table: name.into(),
        offset: 0,
        columns: vec!["Value".into()],
        rows: vec![vec![Cell::scalar(value)]],
        has_more: false,
    }
}

#[test]
fn switching_objects_moves_rows_and_rejects_closed_instance_ids() {
    let mut state = state();
    let a = state.prepare_object_open(table("A")).unwrap().tab;
    assert!(state.set_table_data(data("A", "first")));
    let pointer = state.data.as_ref().unwrap().rows.as_ptr();
    let b = state.prepare_object_open(table("B")).unwrap().tab;
    assert!(state.data.is_none());
    assert!(state.set_table_data(data("B", "second")));
    assert!(state.activate_object(&a));
    assert_eq!(state.data.as_ref().unwrap().rows.as_ptr(), pointer);
    assert_eq!(state.data.as_ref().unwrap().rows[0][0].text, "first");
    assert!(state.close_object(&b));
    let reopened = state.prepare_object_open(table("B")).unwrap().tab;
    assert_ne!(b.id, reopened.id);
    assert!(!state.close_object(&b));
    assert_eq!(state.active_object(), Some(reopened));
}

fn query_target(revision: u64) -> crate::SavedQueryTarget {
    crate::SavedQueryTarget {
        document_revision: revision,
        object_id: 9,
        name: "Q".into(),
    }
}

fn query_object(revision: u64) -> ObjectTarget {
    ObjectTarget {
        document_revision: revision,
        kind: ObjectKind::SavedQuery {
            object_id: 9,
            name: "Q".into(),
        },
    }
}

fn inputs() -> Vec<crate::ParameterInput> {
    vec![crate::ParameterInput {
        name: "Amount".into(),
        kind: "int32".into(),
        text: "7".into(),
        is_null: false,
    }]
}

fn details(kind: &str) -> Vec<crate::ParameterSpec> {
    vec![crate::ParameterSpec {
        name: "Amount".into(),
        kind: kind.into(),
        label: "Amount".into(),
        nullable: false,
    }]
}

fn seeded_query(state: &mut State) -> ObjectTab {
    let tab = state.prepare_object_open(query_object(7)).unwrap().tab;
    state.saved_query.restore_preferences(
        query_target(7),
        Some(crate::workspace::saved_query::InputPreferences {
            inputs: inputs(),
            declarations: vec![("Amount".into(), "int32".into())],
        }),
    );
    state.begin_saved_query_inspection(query_target(7), 1);
    assert!(state.set_saved_query_details(1, query_target(7), details("int32")));
    assert!(state.can_run_saved_query(&query_target(7), &inputs()));
    state.begin_saved_query_run(query_target(7), inputs(), 2);
    let mut result = data("Q", "complete result");
    result.source_revision = None;
    assert!(state.set_saved_query_data(2, query_target(7), result));
    tab
}

#[test]
fn saved_parameters_results_and_scratch_sql_have_independent_owners() {
    let mut state = state();
    let saved = seeded_query(&mut state);
    let saved_rows = state.saved_query.data.as_ref().unwrap().rows.as_ptr();
    let scratch = state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    state.sql = "SELECT 'independent draft';".into();
    let mut result = data("Query results", "scratch result");
    result.source_revision = None;
    state.set_query_data(result);
    let scratch_rows = state.query_data.as_ref().unwrap().rows.as_ptr();
    let table = state.prepare_object_open(table("A")).unwrap().tab;
    assert!(state.set_table_data(data("A", "native row")));
    assert!(state.query_data.is_none());
    assert!(state.saved_query.data.is_none());
    assert!(state.activate_object(&saved));
    assert_eq!(
        state.saved_query.data.as_ref().unwrap().rows.as_ptr(),
        saved_rows
    );
    assert!(state.can_run_saved_query(&query_target(7), &inputs()));
    assert!(state.query_data.is_none());
    assert!(state.activate_object(&scratch));
    assert_eq!(state.sql, "SELECT 'independent draft';");
    assert_eq!(
        state.query_data.as_ref().unwrap().rows.as_ptr(),
        scratch_rows
    );
    assert!(state.activate_object(&table));
    assert_eq!(state.data.as_ref().unwrap().rows[0][0].text, "native row");
}

#[test]
fn restored_layout_keeps_preferences_but_requires_fresh_rows_and_parameter_declarations() {
    let mut state = state();
    let a = state.prepare_object_open(table("A")).unwrap().tab;
    let mut page = data("A", "old row rights");
    page.offset = 100;
    assert!(state.set_table_data(page));
    state.set_schema(crate::Schema {
        table: "A".into(),
        declared_rows: 101,
        fields: vec![],
    });
    assert!(state.prepare_object_design(&a).is_some());
    let q = seeded_query(&mut state);
    let old_tabs = state.object_tabs();
    let layout = state.capture_workspace_layout();
    state.clear_schema();
    state.database.as_mut().unwrap().document_revision = 8;
    assert!(state.restore_workspace_layout(layout, ObjectKind::Table { name: "A".into() }));
    let restored = state.active_object_load().unwrap();
    assert!(restored.needs_load);
    assert!(restored.table_design);
    assert_eq!(restored.offset, 100);
    assert!(state.data.is_none());
    assert!(state.schema.is_none());
    for old in old_tabs {
        assert!(!state.can_activate_object(&old));
        assert!(!state.can_close_object(&old));
    }
    let fresh_q = state
        .object_tabs()
        .into_iter()
        .find(|tab| matches!(tab.target.kind, ObjectKind::SavedQuery { .. }))
        .unwrap();
    assert_ne!(fresh_q.id, q.id);
    assert!(state.activate_object(&fresh_q));
    assert!(state.saved_query.data.is_none());
    state.begin_saved_query_inspection(query_target(8), 3);
    assert!(state.set_saved_query_details(3, query_target(8), details("int32")));
    assert!(state.can_run_saved_query(&query_target(8), &inputs()));

    // Changing the declaration to VALUE must not silently reuse the old LONG
    // input just because VALUE can admit an explicitly chosen LONG carrier.
    let layout = state.capture_workspace_layout();
    state.clear_schema();
    state.database.as_mut().unwrap().document_revision = 9;
    assert!(state.restore_workspace_layout(layout, query_object(9).kind));
    state.begin_saved_query_inspection(query_target(9), 4);
    assert!(state.set_saved_query_details(4, query_target(9), details("value")));
    assert!(!state.can_run_saved_query(&query_target(9), &inputs()));
}

#[test]
fn tab_and_full_capacity_limits_refuse_without_eviction_or_replacing_the_current_page() {
    let mut state = state();
    state.database.as_mut().unwrap().tables = (0..17)
        .map(|index| Table {
            name: format!("T{index}"),
            system: false,
            storage: "Local".into(),
        })
        .collect();
    for index in 0..16 {
        assert!(
            state
                .prepare_object_open(ObjectTarget {
                    document_revision: 7,
                    kind: ObjectKind::Table {
                        name: format!("T{index}")
                    },
                })
                .is_some()
        );
    }
    let previous = state.active_object().unwrap();
    assert!(
        state
            .prepare_object_open(ObjectTarget {
                document_revision: 7,
                kind: ObjectKind::Table { name: "T16".into() },
            })
            .is_none()
    );
    assert_eq!(state.object_tabs().len(), 16);
    assert_eq!(state.active_object(), Some(previous.clone()));
    assert!(state.set_table_data(data("T15", "retained")));
    let rows = state.data.as_ref().unwrap().rows.as_ptr();
    let mut excessive = data("T15", "");
    excessive.rows[0][0].text = String::with_capacity(MAX_RETAINED_BYTES);
    assert!(!state.set_table_data(excessive));
    assert_eq!(state.data.as_ref().unwrap().rows.as_ptr(), rows);
    assert_eq!(state.data.as_ref().unwrap().rows[0][0].text, "retained");
    assert!(state.close_object(&previous));
    assert_eq!(state.object_tabs().len(), 15);
}

#[test]
fn isolated_publishers_do_not_invent_objects_and_managed_publications_cannot_retarget() {
    let mut isolated = State::default();
    isolated.dismiss_backstage();
    assert!(isolated.set_table_data(data("A", "isolated page")));
    let mut result = data("SQL result", "isolated query");
    result.source_revision = None;
    isolated.set_query_data(result);
    assert_eq!(
        isolated.data.as_ref().unwrap().rows[0][0].text,
        "isolated page"
    );
    assert_eq!(
        isolated.query_data.as_ref().unwrap().rows[0][0].text,
        "isolated query"
    );
    assert!(isolated.object_tabs().is_empty());
    assert!(!isolated.can_open_object(&table("A")));

    let mut managed = state();
    let a = managed.prepare_object_open(table("A")).unwrap().tab;
    assert!(managed.set_table_data(data("A", "owned page")));
    assert!(!managed.set_table_data(data("B", "wrong object")));
    let mut query = data("Query result", "wrong kind");
    query.source_revision = None;
    managed.set_query_data(query);
    assert_eq!(managed.active_object(), Some(a));
    assert_eq!(managed.data.as_ref().unwrap().rows[0][0].text, "owned page");
    assert!(managed.query_data.is_none());
}
