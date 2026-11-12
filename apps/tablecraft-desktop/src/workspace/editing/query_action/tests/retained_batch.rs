//! Normal Run retains a complete original batch through No/Yes/save/ACK/reopen.
use super::{
    assert_image, cancel_refused_action, finish_reads, finish_refused_action, finish_save,
    journal_path, opened, reviewed, run,
};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, joined_update, long_column_batch,
};
use tablecraft_ui::{FluentArgs, ObjectKind};

#[test]
fn retained_batch_normal_run_no_then_yes_publishes_both_rows_and_honest_nullable_counts() {
    for (sql, values, changed) in [
        (
            "UPDATE Table1 SET Score=CLng(Nz(Score,0)+5)",
            [Some(22), Some(5)],
            2,
        ),
        ("UPDATE Table1 SET Score=NULL", [None, None], 1),
    ] {
        let source = long_column_batch::two_rows();
        let expected = if values == [Some(22), Some(5)] {
            source.expected
        } else {
            let expected =
                field_changes::update_score(&source.before, source.definition, 0, values[0]);
            let expected = field_changes::update_score(&expected, source.definition, 1, values[1]);
            expected
        };
        let (_directory, path, before, mut harness) = opened(source.before);
        let original = harness.state().state.active_object().unwrap();
        let previous = harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr();
        let discarded = reviewed(&mut harness, sql);
        for (key, count) in [
            ("query-action-matched", 2),
            ("query-action-update-changed", changed),
        ] {
            let mut args = FluentArgs::new();
            args.set("count", count.to_string());
            let label = harness.state().state.localizer.format(key, &args);
            assert!(
                harness
                    .query_by_role_and_label(Role::Label, &label)
                    .is_some()
            );
        }
        assert_image(&std::fs::read(&path).unwrap(), &before);
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
            previous
        );
        assert_image(&std::fs::read(&path).unwrap(), &before);

        reviewed(&mut harness, sql);
        harness
            .get_by_role_and_label(Role::Button, "Yes")
            .click_accesskit();
        harness.run_steps(1);
        let report = finish_save(&mut harness);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.matched_rows == Some(2) && receipt.changed_rows == Some(changed)
            && receipt.schema_change.is_none()));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        harness
            .get_by_role_and_label(Role::Button, "Close")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
        assert!(!harness.state().editing.has_save());
        assert!(!harness.state().state.native_source_stale());
        assert_eq!(harness.state().state.sql, sql);
        let fresh = harness.state().state.active_object().unwrap();
        assert_eq!(fresh.target.kind, ObjectKind::Sql);
        assert_ne!(fresh, original);
        assert!(
            harness.state().state.query_data.is_none(),
            "save does not rerun SQL"
        );
        assert!(!journal_path(&path).exists());
        run(&mut harness, "SELECT ID,Score FROM Table1 ORDER BY ID");
        let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
        assert_eq!(rows.len(), 2);
        for (row, value) in rows.iter().zip(values) {
            assert_eq!(
                row[1].text,
                value.map_or_else(|| "NULL".to_string(), |v| v.to_string())
            );
        }
        assert_image(&std::fs::read(path).unwrap(), &expected);
    }
}

#[test]
fn later_row_assignment_error_preserves_previous_result_and_has_no_saved_prefix() {
    let (_directory, path, before, mut harness) = opened(joined_update::source().bytes);
    let original = harness.state().state.active_object().unwrap();
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let sql = "UPDATE Table1 SET Score=10 \\ (2-ID)";
    assert!(harness.state_mut().state.replace_query_sql(sql.into()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_refused_action(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert_eq!(harness.state().state.sql, sql);
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous
    );
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());
    cancel_refused_action(&mut harness);
    run(&mut harness, "SELECT ID,Score FROM Table1 ORDER BY ID");
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows[0][1].text, "17");
    assert_eq!(rows[1][1].text, "NULL");
    assert_image(&std::fs::read(path).unwrap(), &before);
}
