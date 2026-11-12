//! Same-document saves retain presentation identities, never source authority.
use super::*;

#[test]
fn saved_layout_keeps_tab_ids_and_drafts_with_fresh_empty_sessions() {
    let mut state = state();
    let a = state.prepare_object_open(table("A")).unwrap().tab;
    assert!(state.set_table_data(data("A", "old native rows")));
    let saved = seeded_query(&mut state);
    let sql = state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert!(state.replace_query_sql("SELECT 2 AS Draft".into()));
    let mut result = data("Query results", "old SQL result");
    result.source_revision = None;
    assert!(state.set_query_data(Some("SELECT 1".into()), result));
    let old_tabs = state.object_tabs();
    let layout = state.capture_workspace_layout();

    state.database.as_mut().unwrap().document_revision = 8;
    assert!(state.restore_workspace_layout_after_save(layout, ObjectKind::Sql));
    assert_eq!(state.active_object().unwrap().id, sql.id);
    assert_eq!(state.sql, "SELECT 2 AS Draft");
    assert!(state.query_data.is_none());
    assert!(state.query_pending.is_none());
    assert!(state.data.is_none());
    assert!(state.schema.is_none());
    assert_eq!(state.query_result.retained_bytes(), 0);

    let fresh_tabs = state.object_tabs();
    assert_eq!(fresh_tabs.len(), old_tabs.len());
    for (old, fresh) in old_tabs.iter().zip(&fresh_tabs) {
        assert_eq!(fresh.id, old.id);
        assert_eq!(fresh.target.kind, old.target.kind);
        assert_eq!(fresh.target.document_revision, 8);
        assert!(!state.can_activate_object(old));
        assert!(!state.can_close_object(old));
        assert!(!state.set_object_load_offset(old, 999));
    }

    let fresh_saved = fresh_tabs.iter().find(|tab| tab.id == saved.id).unwrap();
    assert!(state.activate_object(fresh_saved));
    assert!(state.saved_query.data.is_none());
    state.begin_saved_query_inspection(query_target(8), 3);
    assert!(state.set_saved_query_details(3, &query_target(8), details("int32")));
    assert!(state.can_run_saved_query(&query_target(8), &inputs()));

    let fresh_a = fresh_tabs.iter().find(|tab| tab.id == a.id).unwrap();
    assert!(state.activate_object(fresh_a));
    assert!(state.data.is_none());
    assert!(state.active_object_load().unwrap().needs_load);
}

#[test]
fn saved_layout_requires_a_fresh_revision_before_retaining_ids() {
    let mut state = state();
    let tab = state.prepare_object_open(table("A")).unwrap().tab;
    assert!(state.set_table_data(data("A", "unchanged")));
    let rows = state.data.as_ref().unwrap().rows.as_ptr();
    let layout = state.capture_workspace_layout();

    assert!(!state.restore_workspace_layout_after_save(layout, ObjectKind::Sql));
    assert_eq!(state.active_object(), Some(tab));
    assert_eq!(state.data.as_ref().unwrap().rows.as_ptr(), rows);
}

#[test]
fn saved_layout_missing_object_refuses_without_publishing_partial_tabs() {
    let mut state = state();
    let tab = state.prepare_object_open(table("A")).unwrap().tab;
    assert!(state.set_table_data(data("A", "unchanged")));
    let layout = state.capture_workspace_layout();
    state.database.as_mut().unwrap().document_revision = 8;
    state.database.as_mut().unwrap().tables.clear();

    assert!(!state.restore_workspace_layout_after_save(layout, ObjectKind::Sql));
    assert_eq!(state.objects.entries.len(), 1);
    assert_eq!(state.objects.entries[0].tab, tab);
    assert_eq!(state.data.as_ref().unwrap().rows[0][0].text, "unchanged");
}
