//! Exact deferred table closure preserves the parked raw SQL owner.
use super::{choose, pending};
use crate::workspace::test_support::objects::{EXACT_SCRATCH, scratch};
use crate::workspace::test_support::{finish_reads, finish_schema_save, opened};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field as raw;

#[test]
fn dirty_close_cancel_then_save_or_discard_keeps_the_exact_scratch_owner() {
    for decision in ["Save", "Discard"] {
        let before = raw::source(raw::SourceCase::NumberHistory);
        let after = raw::remove_nonprimary(&before, "Score", raw::Variant::AppendedGroupRemoved);
        let (_directory, path, _, mut harness) = opened(before.clone());
        let table = harness.state().state.active_object().unwrap();
        let scratch = scratch(&mut harness, &table);
        pending(&mut harness, "Score");

        harness.state_mut().close_object(table.clone());
        harness.run_steps(2);
        assert!(harness.state().state.field_drop_close_open());
        assert!(harness.state().deferred_draft_action.is_some());
        choose(&mut harness, "field_drop_close", "Cancel");

        assert!(!harness.state().state.field_drop_close_open());
        assert!(harness.state().deferred_draft_action.is_none());
        assert!(harness.state().state.field_drop_dirty());
        assert!(harness.state().state.object_tab_is_current(&table));
        assert!(!harness.state().editing.has_save());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());

        harness.state_mut().close_object(table.clone());
        harness.run_steps(2);
        choose(&mut harness, "field_drop_close", decision);

        if decision == "Save" {
            assert!(harness.state().state.object_tab_is_current(&table));
            assert!(harness.state().deferred_draft_action.is_some());
            finish_schema_save(&mut harness);
            assert_eq!(std::fs::read(&path).unwrap(), after);
        } else {
            finish_reads(&mut harness);
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }

        assert!(!harness.state().state.field_drop_open());
        assert!(!harness.state().state.field_drop_dirty());
        assert!(harness.state().deferred_draft_action.is_none());
        assert!(!harness.state().editing.has_save());
        assert!(!journal_path(&path).exists());
        let tabs = harness.state().state.object_tabs();
        assert_eq!(tabs.len(), 1);
        assert!(tabs.iter().all(|tab| tab.id != table.id));
        let retained = tabs.iter().find(|tab| tab.id == scratch.id).unwrap();
        assert_eq!(retained.target.kind, scratch.target.kind);
        harness.state_mut().activate_object(retained.clone());
        finish_reads(&mut harness);
        assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    }
}
