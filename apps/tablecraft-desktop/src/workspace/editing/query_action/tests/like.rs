//! Normal Run keeps LIKE policy evaluation before confirmation and native writes.
use super::{
    assert_image, finish_reads, finish_refused_action, finish_save, journal_path, opened, reviewed,
    run,
};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{currency::mixed, fields, joined_update},
    database_properties,
};

fn source(mode: i32) -> (Vec<u8>, u32) {
    let source = joined_update::source();
    let properties = database_properties::name_auto_correct_disabled_with_query_mode(mode);
    (
        database_properties::with_native17_properties(&source.bytes, &properties),
        source.target_definition,
    )
}

#[test]
fn normal_run_like_update_observes_database_mode_no_yes_save_ack_and_reopen() {
    for (mode, pattern) in [(0_i32, "a*"), (1_i32, "a%")] {
        let seed = fields::source(fields::Case::FirstNumber).bytes;
        let definition = (seed.len() / 4096) as u32;
        let mut before = mixed::append_table(&seed, "Measurements");
        for _ in 0..2 {
            before = mixed::append(&before, definition);
        }
        before = mixed::update_number(&before, definition, 0, Some(17));
        before = mixed::update_currency(&before, definition, 0, Some(12345));
        before = mixed::update_text(&before, definition, 0, Some("Apple"));
        before = mixed::update_number(&before, definition, 1, None);
        let properties = database_properties::name_auto_correct_disabled_with_query_mode(mode);
        let before = database_properties::with_native17_properties(&before, &properties);
        let expected = mixed::update_number(&before, definition, 0, Some(25));
        let (_directory, path, before, mut harness) = opened(before);
        let prior = harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr();
        let sql = format!("UPDATE Measurements SET Score=25 WHERE Label LIKE '{pattern}'");
        reviewed(&mut harness, &sql);
        assert_image(&std::fs::read(&path).unwrap(), &before);
        assert!(!journal_path(&path).exists());
        harness
            .get_by_role_and_label(Role::Button, "No")
            .click_accesskit();
        harness.run_steps(2);
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
            prior
        );
        assert_image(&std::fs::read(&path).unwrap(), &before);
        reviewed(&mut harness, &sql);
        harness
            .get_by_role_and_label(Role::Button, "Yes")
            .click_accesskit();
        harness.run_steps(1);
        let report = finish_save(&mut harness);
        assert!(
            matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.matched_rows == Some(1) && receipt.changed_rows == Some(1)
                && receipt.schema_change.is_none()),
            "{report:?}"
        );
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        harness
            .get_by_role_and_label(Role::Button, "Close")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
        assert!(!harness.state().editing.has_save());
        assert!(!journal_path(&path).exists());
        run(
            &mut harness,
            "SELECT ID,Score,Amount,Label FROM Measurements ORDER BY ID",
        );
        let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
        assert_eq!(rows[0][1].text, "25");
        assert_eq!(rows[1][1].text, "NULL");
        assert_eq!(rows[0][2].text, "1.2345");
        assert_eq!(rows[0][3].text, "Apple");
        assert_image(&std::fs::read(&path).unwrap(), &expected);
    }
}

#[test]
fn like_late_error_and_unknown_mode_retain_previous_result_without_review_or_save() {
    for mode in [0, 1, 2] {
        let (before, _) = source(mode);
        let (_directory, path, before, mut harness) = opened(before);
        let prior = harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr();
        let sql = if mode == 2 {
            "UPDATE Table1 SET Score=25 WHERE '17' LIKE '1*'".into()
        } else {
            let pattern = if mode == 0 { "1*" } else { "1%" };
            format!("UPDATE Table1 SET Score=25 WHERE '17' LIKE IIf(ID=2,10 \\ (2-ID),'{pattern}')")
        };
        assert!(harness.state_mut().state.replace_query_sql(sql));
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Button, "Run query")
            .click_accesskit();
        harness.run_steps(1);
        finish_refused_action(&mut harness);
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
            prior
        );
        assert_image(&std::fs::read(&path).unwrap(), &before);
        assert!(!journal_path(&path).exists());
    }
}
