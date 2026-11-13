//! Actual SQL decisions and schema publication retain index history and prior results.
use super::{assert_image, close_target, finish_reads, finish_save, journey, opened, reviewed};
use crate::workspace::Desktop;
use crate::workspace::test_support::objects::discard_query_close_for_open;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::{Command, SaveOutcome, SaveSchemaChange, journal_path};
use tablecraft_testkit::fixtures::access::secondary_index::normal::{
    self as raw, Population, dropping, history,
};

#[test]
fn drop_index_no_then_yes_preserves_results_and_generated_primary_or_surviving_alias() {
    for descending in [false, true] {
        for source in [
            raw::source(Population::NullableDuplicate, descending),
            raw::protected_companion(descending),
        ] {
            let expected = dropping::after(&source);
            let sql = format!("DROP INDEX [{}] ON [{}]", source.index_name, source.table);
            journey(
                source.expected,
                expected,
                &sql,
                source.table,
                SaveSchemaChange::IndexDropped,
            );
        }
    }
    let source = raw::protected_companion(false);
    let alias = dropping::surviving_alias(&source, "ScoreAlias");
    journey(
        alias.before,
        alias.expected,
        &format!("DROP INDEX [{}] ON [{}]", source.index_name, source.table),
        source.table,
        SaveSchemaChange::IndexDropped,
    );
}

fn save_step(harness: &mut Harness<'_, Desktop>, sql: &str, change: SaveSchemaChange) {
    reviewed(harness, sql);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    assert!(
        matches!(finish_save(harness).outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(change)
        && receipt.matched_rows.is_none() && receipt.changed_rows.is_none())
    );
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.native_source_stale());
    assert!(
        harness.state().state.query_data.is_none(),
        "schema save must not rerun SQL"
    );
}

#[test]
fn alias_drop_add_and_fresh_index_create_complete_through_normal_sql_run() {
    for descending in [false, true] {
        let source = raw::protected_companion(false);
        let alias = dropping::surviving_alias(&source, "ScoreAlias");
        let expected = history::source_with_direction(descending);
        let (_directory, path, _, mut harness) = opened(alias.before);
        close_target(&mut harness, source.table);
        let sql_tab = harness.state().state.active_object().unwrap();
        save_step(
            &mut harness,
            &format!("DROP INDEX [{}] ON [{}]", source.index_name, source.table),
            SaveSchemaChange::IndexDropped,
        );
        assert_image(&std::fs::read(&path).unwrap(), &alias.expected);
        save_step(
            &mut harness,
            &format!("ALTER TABLE [{}] ADD COLUMN OtherScore LONG", source.table),
            SaveSchemaChange::ColumnAdded,
        );
        assert_image(&std::fs::read(&path).unwrap(), &expected.before);
        let sql = format!(
            "CREATE INDEX OtherLookup ON [{}] (OtherScore {})",
            source.table,
            if descending { "DESC" } else { "ASC" }
        );
        save_step(&mut harness, &sql, SaveSchemaChange::IndexCreated);
        assert_image(&std::fs::read(&path).unwrap(), &expected.expected);
        assert_eq!(harness.state().state.sql, sql);
        assert_eq!(
            harness.state().state.active_object().unwrap().id,
            sql_tab.id
        );
        assert!(!journal_path(&path).exists());
    }
}

#[test]
fn changed_sql_or_reopened_revision_cannot_apply_an_old_drop_confirmation() {
    for reopen in [false, true] {
        let source = raw::protected_companion(false);
        let (_directory, path, original, mut harness) = opened(source.expected);
        close_target(&mut harness, source.table);
        let sql = format!("DROP INDEX [{}] ON [{}]", source.index_name, source.table);
        let id = reviewed(&mut harness, &sql);
        if reopen {
            let revision = harness
                .state()
                .state
                .database
                .as_ref()
                .unwrap()
                .document_revision;
            let generation = harness.state().generation;
            // NO_CLONE: This attempted Open and the later confirmed Open each
            // own a path while the independent file observer remains available.
            harness.state_mut().open_request(Command::Open {
                path: path.clone(),
                password: String::new(),
            });
            assert_eq!(harness.state().generation, generation);
            assert!(harness.state().state.query_action_dialog_open());
            assert!(harness.state().deferred_draft_action.is_none());
            assert!(!harness.state().editing.has_save());
            assert_eq!(std::fs::read(&path).unwrap(), original);
            // A modal action decision must be resolved before a global Open can
            // ask to discard the still-unsaved SQL. Retain the old issued ID.
            harness
                .get_by_role_and_label(Role::Button, "No")
                .click_accesskit();
            harness.run_steps(2);
            assert!(!harness.state().state.query_action_dialog_open());
            // NO_CLONE: The accepted global Open owns its selected path; the
            // test keeps the same file to verify that no stale action can write.
            harness.state_mut().open_request(Command::Open {
                path: path.clone(),
                password: String::new(),
            });
            assert_eq!(harness.state().generation, generation);
            discard_query_close_for_open(&mut harness);
            finish_reads(&mut harness);
            assert!(harness.state().generation > generation);
            assert!(
                harness
                    .state()
                    .state
                    .database
                    .as_ref()
                    .unwrap()
                    .document_revision
                    > revision
            );
        } else {
            assert!(
                harness
                    .state_mut()
                    .state
                    .replace_query_sql("SELECT 2".into())
            );
            harness.run_steps(2);
        }
        harness.state_mut().apply_query_action(id);
        harness.run_steps(2);
        assert!(!harness.state().editing.has_save());
        assert_image(&std::fs::read(&path).unwrap(), &original);
        assert!(!journal_path(&path).exists());
    }
}
