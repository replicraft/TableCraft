use super::inspected;
use crate::{Data, ObjectKind, ObjectTarget, Table};

#[test]
fn parked_dirty_sql_blocks_cell_edits_until_the_exact_draft_is_discarded() {
    let (mut state, _) = inspected();
    let saved_tab = state.active_object().unwrap();
    state.database.as_mut().unwrap().tables.push(Table {
        name: "Items".into(),
        local: true,
        system: false,
        storage: "Local".into(),
    });
    let table = ObjectTarget {
        document_revision: 7,
        kind: ObjectKind::Table {
            name: "Items".into(),
        },
    };
    let data = || Data {
        source_revision: Some(7),
        row_ordinals: None,
        table: "Items".into(),
        offset: 0,
        columns: vec!["Amount".into()],
        rows: vec![vec!["12".into()]],
        has_more: false,
    };

    state.prepare_object_open(table.clone()).unwrap();
    assert!(state.set_table_data(data()));
    let target = state.data.as_ref().unwrap().edit_target(0, 0).unwrap();
    assert!(state.can_edit_cell(&target));
    assert!(state.can_restart_for_update());

    assert!(state.activate_object(&saved_tab));
    assert!(state.set_saved_query_sql_text("SELECT 43 AS Answer".into()));
    let revision = state.saved_query_sql_revision_for(&saved_tab).unwrap();
    state.prepare_object_open(table).unwrap();
    assert!(state.set_table_data(data()));
    assert!(state.has_modified_saved_query_sql());
    assert!(!state.can_restart_for_update());
    assert!(!state.can_edit_cell(&target));

    assert!(!state.discard_saved_query_sql(&saved_tab, revision + 1));
    assert!(!state.can_edit_cell(&target));
    assert!(state.discard_saved_query_sql(&saved_tab, revision));
    assert!(state.can_edit_cell(&target));
    assert!(state.can_restart_for_update());
}
