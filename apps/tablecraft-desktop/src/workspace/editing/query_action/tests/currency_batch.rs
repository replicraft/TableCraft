//! Normal SQL Run keeps Currency batches atomic across review and fresh reopen.
use super::{
    assert_image, cancel_refused_action, finish_reads, finish_refused_action, finish_save,
    journal_path, opened, reviewed, run,
};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::currency::batch;
use tablecraft_ui::FluentArgs;

#[test]
fn currency_batch_normal_run_no_yes_null_and_zero_keep_complete_native_images() {
    for (sql, values, matched, changed) in [
        (
            "UPDATE CurrencyValues SET KeyValue=CCur(Nz(KeyValue,0)+1.25)",
            [Some(24845), Some(12500)],
            2,
            2,
        ),
        (
            "UPDATE CurrencyValues SET KeyValue=NULL",
            [None, None],
            2,
            1,
        ),
        (
            "UPDATE CurrencyValues SET KeyValue=CCur(5.25) WHERE KeyValue>CCur(100)",
            [Some(12345), None],
            0,
            0,
        ),
    ] {
        let source = batch::values(&[Some(12345), None], &values);
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
            ("query-action-matched", matched),
            ("query-action-update-changed", changed),
        ] {
            let mut arguments = FluentArgs::new();
            arguments.set("count", count.to_string());
            let label = harness.state().state.localizer.format(key, &arguments);
            harness.get_by_role_and_label(Role::Label, &label);
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
            harness.state().state.active_object().as_ref(),
            Some(&original)
        );
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
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("Currency save must complete: {report:?}");
        };
        assert_eq!(
            (receipt.matched_rows, receipt.changed_rows),
            (Some(matched), Some(changed))
        );
        assert!(receipt.schema_change.is_none());
        assert_image(&std::fs::read(&path).unwrap(), &source.expected);
        harness
            .get_by_role_and_label(Role::Button, "Close")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
        assert!(!harness.state().editing.has_save());
        assert!(!harness.state().state.native_source_stale());
        let fresh = harness.state().state.active_object().unwrap();
        assert_eq!(fresh.id, original.id);
        assert_eq!(fresh.target.kind, original.target.kind);
        assert_ne!(
            fresh.target.document_revision,
            original.target.document_revision
        );
        assert!(!harness.state().state.can_activate_object(&original));
        assert!(harness.state().state.query_data.is_none());
        assert_eq!(harness.state().state.sql, sql);
        assert!(!journal_path(&path).exists());

        run(
            &mut harness,
            "SELECT IIf(IsNull(KeyValue),NULL,CLng(Nz(KeyValue,0)*10000)) AS ScaledUnits, Amount, Label FROM CurrencyValues ORDER BY Guard",
        );
        let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
        assert_eq!(rows.len(), values.len());
        for (row, value) in rows.iter().zip(values) {
            assert_eq!(
                row[0].text,
                value.map_or_else(|| "NULL".to_string(), |value| value.to_string())
            );
        }
        assert_image(&std::fs::read(path).unwrap(), &source.expected);
    }
}

#[test]
fn later_currency_overflow_retains_the_prior_result_and_every_original_byte() {
    let source = batch::values(&[Some(0), Some(i64::MAX)], &[Some(0), Some(i64::MAX)]);
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
    let sql = "UPDATE CurrencyValues SET KeyValue=KeyValue+CCur(1)";
    assert!(harness.state_mut().state.replace_query_sql(sql.into()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_refused_action(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.active_object(), Some(original));
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
    assert_eq!(harness.state().state.sql, sql);
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());
    cancel_refused_action(&mut harness);
    run(&mut harness, "SELECT 99 AS StillHealthy");
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );
    assert_image(&std::fs::read(path).unwrap(), &before);
}
