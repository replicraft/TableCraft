//! A newly initialized query remains editable through ordinary TCQE save.
use super::{
    Case, ObjectKind, ObjectTarget, accept_name, finish_reads, finish_schema_save, name,
    opened_source, raw, run_saved, scratch, sql,
};
use egui::{Key, Modifiers};
use tablecraft_core::values::LegacyDateTime;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::query_expression_edits;
use tablecraft_ui::SavedQuerySqlEligibility;

fn ordinary_save(case: Case) {
    let at = LegacyDateTime::from_ole_days(45000.5).unwrap();
    let source = raw::source(case, "Created", "Answer", 99, at);
    let data_page = source.before.len() / 4096 + 1;
    let expected = query_expression_edits::replace_on_page(&source.expected, data_page, 3, "101");
    let (_directory, path, source, clock, mut harness) = opened_source(source);
    scratch(&mut harness);
    let created_sql = " select  99 AS Answer;\n";
    sql(&mut harness, created_sql);
    let created_epoch = harness
        .state()
        .state
        .query_sql_revision()
        .unwrap()
        .checked_add(1)
        .unwrap();
    name(&mut harness, false);
    accept_name(&mut harness);
    finish_schema_save(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), source.expected);
    assert_eq!(clock.calls(), 1);
    let (original, proposed, epoch, _) = harness.state().state.saved_query_sql().unwrap();
    assert_eq!(original, created_sql);
    assert_eq!(proposed, created_sql);
    assert_eq!(
        epoch, created_epoch,
        "real eligibility inspection cannot rewrite the authored text epoch"
    );
    let created = harness.state().state.active_object().unwrap();
    assert!(
        matches!(&created.target.kind, ObjectKind::SavedQuery { name, .. } if name == "Created")
    );
    assert_eq!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .queries
            .len(),
        1
    );
    assert_eq!(
        harness.state().state.saved_query_sql_eligibility(),
        Some(SavedQuerySqlEligibility::QualifiedExpressionSelect),
        "successful creation must not be mistaken for ordinary edit eligibility"
    );

    // Actual SQL editing and ordinary Save must use the existing expression
    // review, not Name/another creation. No creation timestamp is sampled again.
    let authored = "SELECT 101 AS [Answer]";
    sql(&mut harness, authored);
    assert_eq!(harness.state().state.saved_query_sql().unwrap().1, authored);
    assert!(harness.state().state.saved_query_sql_dirty());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    assert!(!harness.state().state.query_creation_name_open());
    finish_schema_save(&mut harness);
    let fresh = harness.state().state.active_object().unwrap();
    assert_eq!(fresh.id, created.id);
    assert_eq!(fresh.target.kind, created.target.kind);
    assert!(fresh.target.document_revision > created.target.document_revision);
    assert!(!harness.state().state.can_activate_object(&created));
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.saved_query_sql_dirty());
    assert!(
        harness
            .state()
            .state
            .saved_query_sql_save_status()
            .is_none()
    );
    assert!(harness.state().state.query_creation_save_status().is_none());
    assert_eq!(harness.state().state.saved_query_sql().unwrap().0, authored);
    assert_eq!(harness.state().state.saved_query_sql().unwrap().1, authored);
    assert_eq!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .queries
            .len(),
        1
    );
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
    assert_eq!(clock.calls(), 1);
    run_saved(&mut harness, "101");

    if matches!(case, Case::Ordinary) {
        harness.state_mut().open_object(ObjectTarget {
            document_revision: fresh.target.document_revision,
            kind: ObjectKind::Table {
                name: "Items".into(),
            },
        });
        harness.run_steps(1);
        finish_reads(&mut harness);
        let data = harness.state().state.data.as_ref().unwrap();
        assert_eq!(data.rows.len(), 1);
        assert_eq!(data.rows[0][0].text, "1");
        assert_eq!(data.rows[0][1].text, "10");
    }
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(path).unwrap(), expected);
}

#[test]
fn initialized_blank_created_query_can_be_authored_and_saved_through_ordinary_tcqe() {
    ordinary_save(Case::Blank);
}

#[test]
fn initialized_created_query_ordinary_save_protects_the_existing_local_table() {
    ordinary_save(Case::Ordinary);
}
