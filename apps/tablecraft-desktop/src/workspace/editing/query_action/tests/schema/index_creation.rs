//! Actual SQL Run, No/Yes, native save and guarded unchanged-catalog publication.
use super::{assert_image, finish_reads, finish_save, opened, reviewed};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::{SaveOutcome, SaveSchemaChange, journal_path};
use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, Population};
use tablecraft_ui::FluentArgs;

#[test]
fn create_index_run_preserves_source_on_no_then_saves_one_schema_only_operation() {
    for descending in [false, true] {
        let source = raw::source(Population::NullableDuplicate, descending);
        let expected = source.expected;
        let sql = format!(
            "CREATE INDEX [{}] ON [{}] ([{}] {})",
            source.index_name,
            source.table,
            source.column,
            if descending { "DESC" } else { "ASC" }
        );
        let (_directory, path, original, mut harness) = opened(source.before);
        let sql_tab = harness.state().state.active_object().unwrap();
        let first = reviewed(&mut harness, &sql);
        for (key, name) in [
            ("query-action-schema-index-name", source.index_name),
            ("query-action-schema-index-field", source.column),
        ] {
            let mut arguments = FluentArgs::new();
            arguments.set("name", name);
            let label = harness.state().state.localizer.format(key, &arguments);
            assert!(
                harness
                    .query_by_role_and_label(Role::Label, &label)
                    .is_some()
            );
        }
        let direction = harness.state().state.localizer.text(if descending {
            "query-action-schema-index-descending"
        } else {
            "query-action-schema-index-ascending"
        });
        assert!(
            harness
                .query_by_role_and_label(Role::Label, &direction)
                .is_some()
        );
        let mut arguments = FluentArgs::new();
        arguments.set("count", "6");
        let population = harness
            .state()
            .state
            .localizer
            .format("query-action-schema-index-source-rows", &arguments);
        assert!(
            harness
                .query_by_role_and_label(Role::Label, &population)
                .is_some()
        );
        let matched = harness
            .state()
            .state
            .localizer
            .format("query-action-matched", &arguments);
        assert!(
            harness
                .query_by_role_and_label(Role::Label, &matched)
                .is_none()
        );
        assert_image(&std::fs::read(&path).unwrap(), &original);
        assert!(!journal_path(&path).exists());
        harness
            .get_by_role_and_label(Role::Button, "No")
            .click_accesskit();
        harness.run_steps(2);
        harness.state_mut().apply_query_action(first);
        assert!(!harness.state().editing.has_save());
        assert_image(&std::fs::read(&path).unwrap(), &original);

        reviewed(&mut harness, &sql);
        harness
            .get_by_role_and_label(Role::Button, "Yes")
            .click_accesskit();
        harness.run_steps(1);
        let report = finish_save(&mut harness);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.schema_change == Some(SaveSchemaChange::IndexCreated)
            && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        harness
            .get_by_role_and_label(Role::Button, "Close")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
        assert!(!harness.state().editing.has_save());
        assert!(!harness.state().state.native_source_stale());
        assert_eq!(harness.state().state.sql, sql);
        assert_eq!(
            harness.state().state.active_object().unwrap().id,
            sql_tab.id
        );
        assert!(!journal_path(&path).exists());
        assert_image(&std::fs::read(path).unwrap(), &expected);
    }
}

mod numeric;
