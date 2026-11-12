//! Normal SQL Run retains the same warning/save/acknowledgement for numeric keys.
use super::{assert_image, finish_reads, finish_save, opened, reviewed};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::{SaveOutcome, SaveSchemaChange, journal_path};
use tablecraft_testkit::fixtures::access::secondary_index::numeric::{
    self as raw, Kind, Population,
};
use tablecraft_ui::FluentArgs;

#[test]
fn numeric_index_normal_run_no_then_yes_preserves_source_and_schema_only_receipt() {
    for (kind, descending) in [
        (Kind::Boolean, false),
        (Kind::Currency, true),
        (Kind::Int64, false),
    ] {
        let source = raw::source(kind, Population::NullableDuplicate, descending);
        let sql = format!(
            "CREATE INDEX [{}] ON [{}] ([{}] {})",
            source.index_name,
            source.table,
            source.column,
            if descending { "DESC" } else { "ASC" },
        );
        let (_directory, path, original, mut harness) = opened(source.before);
        let tab = harness.state().state.active_object().unwrap().id;
        let previous_rows = harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr();
        let discarded = reviewed(&mut harness, &sql);
        for (key, name) in [
            ("query-action-schema-index-name", source.index_name),
            ("query-action-schema-index-field", source.column),
        ] {
            let mut args = FluentArgs::new();
            args.set("name", name);
            let label = harness.state().state.localizer.format(key, &args);
            assert!(
                harness
                    .query_by_role_and_label(Role::Label, &label)
                    .is_some()
            );
        }
        let mut args = FluentArgs::new();
        args.set("count", source.rows.len().to_string());
        let matched = harness
            .state()
            .state
            .localizer
            .format("query-action-matched", &args);
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
        harness.state_mut().apply_query_action(discarded);
        assert!(!harness.state().editing.has_save());
        assert_eq!(
            harness
                .state()
                .state
                .query_data
                .as_ref()
                .unwrap()
                .rows
                .as_ptr(),
            previous_rows
        );
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
        assert_image(&std::fs::read(&path).unwrap(), &source.expected);
        harness
            .get_by_role_and_label(Role::Button, "Close")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
        assert!(!harness.state().editing.has_save());
        assert!(!harness.state().state.native_source_stale());
        assert!(
            harness.state().state.query_data.is_none(),
            "DDL does not rerun SQL"
        );
        assert_eq!(harness.state().state.sql, sql);
        assert_eq!(harness.state().state.active_object().unwrap().id, tab);
        assert!(!journal_path(&path).exists());
        assert_image(&std::fs::read(path).unwrap(), &source.expected);
    }
}
