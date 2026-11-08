//! Real witnessed saves preserve the workspace without restoring old row authority.
use super::{query, storage_pair};
use crate::workspace::test_support::{finish_reads, finish_save, opened};
use tablecraft_runtime::SaveOutcome;
use tablecraft_ui::{ObjectKind, ObjectTarget};

#[test]
fn witnessed_query_save_preserves_other_tab_identities_and_exact_scratch_sql() {
    let [before, after] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before);
    query(&mut harness);
    let saved = harness.state().state.active_object().unwrap();
    let revision = saved.target.document_revision;

    harness.state_mut().open_object(ObjectTarget {
        document_revision: revision,
        kind: ObjectKind::Sql,
    });
    let scratch = harness.state().state.active_object().unwrap();
    let scratch_text = "-- retain this independent editor\n SELECT 99 AS [Scratch];\n";
    harness.state_mut().state.sql = scratch_text.into();

    // This independent nine-page fixture has only system tables. Opening its
    // real catalog still exercises table-session invalidation across the save.
    harness.state_mut().open_object(ObjectTarget {
        document_revision: revision,
        kind: ObjectKind::Table {
            name: "MSysObjects".into(),
        },
    });
    finish_reads(&mut harness);
    let table = harness.state().state.active_object().unwrap();
    assert!(harness.state().state.data.is_some());
    harness.state_mut().activate_object(saved.clone());
    finish_reads(&mut harness);
    let tabs = harness.state().state.object_tabs();
    let request = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    let proposed = request.proposed_sql.clone();

    harness.state_mut().save_query_definition(request);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    harness.state_mut().acknowledge_query_definition(report.id);
    finish_reads(&mut harness);

    let state = &harness.state().state;
    let fresh_revision = state.database.as_ref().unwrap().document_revision;
    assert!(fresh_revision > revision);
    let fresh_tabs = state.object_tabs();
    assert_eq!(fresh_tabs.len(), tabs.len());
    for (old, fresh) in tabs.iter().zip(&fresh_tabs) {
        assert_eq!(fresh.id, old.id);
        assert_eq!(fresh.target.kind, old.target.kind);
        assert_eq!(fresh.target.document_revision, fresh_revision);
    }
    assert_eq!(state.active_object().unwrap().id, saved.id);
    assert!(!state.can_activate_object(&saved));
    assert!(!state.can_close_object(&table));
    assert_eq!(state.saved_query_sql().unwrap().1, proposed);
    assert!(!state.saved_query_sql_dirty());
    assert_eq!(state.sql, scratch_text);
    assert!(state.data.is_none());
    assert!(state.query_data.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), after);

    let target = state.selected_saved_target().unwrap().clone();
    harness.state_mut().run_saved_query(target, Vec::new());
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.selected_cell_text(), Some("43"));
    assert!(!harness.state().state.saved_query_sql_dirty());

    let scratch = fresh_tabs.iter().find(|tab| tab.id == scratch.id).unwrap();
    harness.state_mut().activate_object(scratch.clone());
    assert_eq!(harness.state().state.sql, scratch_text);
    assert!(harness.state().state.query_data.is_none());
    let table = fresh_tabs.iter().find(|tab| tab.id == table.id).unwrap();
    harness.state_mut().activate_object(table.clone());
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().source_revision,
        Some(fresh_revision)
    );
    assert_eq!(std::fs::read(&path).unwrap(), after);
}
