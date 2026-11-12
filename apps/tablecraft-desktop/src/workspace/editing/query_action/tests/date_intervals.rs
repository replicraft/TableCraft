//! SQL date intervals reach the actual headless review/save workflow without new write authority.
use super::{
    assert_image, cancel_refused_action, finish_reads, finish_refused_action, finish_save,
    journal_path, opened, reviewed, run,
};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, joined_update,
};

#[test]
fn date_intervals_update_no_then_yes_saves_one_complete_original_bound_image() {
    let source = joined_update::source();
    let expected = field_changes::update_score(&source.bytes, source.target_definition, 0, Some(4));
    let expected = field_changes::update_score(&expected, source.target_definition, 1, Some(5));
    let (_directory, path, before, mut harness) = opened(source.bytes);
    let sql = "UPDATE Table1 SET Score=CLng(DatePart('m',DateAdd('m',ID,#2024-01-31#))+DateDiff('d',#2024-02-28#,#2024-03-01#))";
    let discarded = reviewed(&mut harness, sql);
    assert_image(&std::fs::read(&path).unwrap(), &before);
    harness
        .get_by_role_and_label(Role::Button, "No")
        .click_accesskit();
    harness.run_steps(2);
    harness.state_mut().apply_query_action(discarded);
    assert!(!harness.state().editing.has_save());
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());

    reviewed(&mut harness, sql);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows == Some(2) && receipt.changed_rows == Some(2)
        && receipt.schema_change.is_none()));
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(!journal_path(&path).exists());
    run(&mut harness, "SELECT ID,Score FROM Table1 ORDER BY ID");
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows[0][1].text, "4");
    assert_eq!(rows[1][1].text, "5");
    assert_image(&std::fs::read(path).unwrap(), &expected);
}

#[test]
fn late_invalid_interval_or_date_domain_preserves_previous_results_and_all_bytes() {
    for sql in [
        "UPDATE Table1 SET Score=DatePart(IIf(ID=1,'d','invalid'),#2024-01-01#)",
        "UPDATE Table1 SET Score=Year(DateAdd('yyyy',1-ID,#0100-01-01#))",
    ] {
        let (_directory, path, before, mut harness) = opened(joined_update::source().bytes);
        let previous = harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr();
        assert!(harness.state_mut().state.replace_query_sql(sql.into()));
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Button, "Run query")
            .click_accesskit();
        harness.run_steps(1);
        finish_refused_action(&mut harness);
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
}

#[test]
fn correlated_date_exists_and_in_reach_real_review_save_and_fresh_rows() {
    for sql in [
        "UPDATE Table1 AS t SET Score=25 WHERE EXISTS (SELECT DatePart('m',DateAdd('m',1,#2024-01-31#)) FROM Changes AS c WHERE c.Id=t.ID AND DateDiff('d',#2024-01-01#,#2024-01-02#)=1 AND Day(CDate(#2024-01-01#))=1 AND c.Score=5)",
        "UPDATE Table1 AS t SET Score=25 WHERE t.ID IN (SELECT CLng(DatePart('d',DateAdd('d',c.Id-1,#2024-01-01#))) FROM Changes AS c WHERE c.Id=t.ID AND c.Score=5)",
    ] {
        let source = joined_update::source();
        let expected =
            field_changes::update_score(&source.bytes, source.target_definition, 0, Some(25));
        let (_directory, path, before, mut harness) = opened(source.bytes);
        reviewed(&mut harness, sql);
        assert_image(&std::fs::read(&path).unwrap(), &before);
        harness
            .get_by_role_and_label(Role::Button, "Yes")
            .click_accesskit();
        harness.run_steps(1);
        let report = finish_save(&mut harness);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.matched_rows == Some(1) && receipt.changed_rows == Some(1)));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        harness
            .get_by_role_and_label(Role::Button, "Close")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
        run(&mut harness, "SELECT ID,Score FROM Table1 ORDER BY ID");
        let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
        assert_eq!(rows[0][1].text, "25");
        assert_eq!(rows[1][1].text, "NULL");
        assert_image(&std::fs::read(path).unwrap(), &expected);
    }
}

#[test]
fn later_correlated_date_failure_preserves_previous_results_and_every_source_byte() {
    let (_directory, path, before, mut harness) = opened(joined_update::source().bytes);
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let sql = "UPDATE Table1 AS t SET Score=25 WHERE EXISTS (SELECT DatePart(IIf(t.ID=1,'d','invalid'),#2024-01-01#) FROM Changes AS c WHERE c.Id=t.ID)";
    assert!(harness.state_mut().state.replace_query_sql(sql.into()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_refused_action(&mut harness);
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
