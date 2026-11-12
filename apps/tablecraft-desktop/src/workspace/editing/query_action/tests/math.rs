//! SQL math reaches the actual headless review/save workflow without new write authority.
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
fn math_update_no_then_yes_saves_one_complete_original_bound_image() {
    let source = joined_update::source();
    let expected = field_changes::update_score(&source.bytes, source.target_definition, 0, Some(2));
    let expected = field_changes::update_score(&expected, source.target_definition, 1, Some(4));
    let (_directory, path, before, mut harness) = opened(source.bytes);
    let sql = "UPDATE Table1 SET Score=CLng(Sqr(ID*ID*4))";
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
    assert_eq!(rows[0][1].text, "2");
    assert_eq!(rows[1][1].text, "4");
    assert_image(&std::fs::read(path).unwrap(), &expected);
}

#[test]
fn late_math_domain_or_integral_overflow_preserves_previous_results_and_all_bytes() {
    for sql in [
        "UPDATE Table1 SET Score=CLng(Sqr(2-ID*2))",
        "UPDATE Table1 SET Score=CLng(Exp(ID*11))",
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
