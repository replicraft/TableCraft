//! Real Run reviews six exact fixed kinds without inferring types from NULL values.
use super::{
    assert_image, cancel_refused_action, finish_reads, finish_refused_action, finish_save,
    journal_path, opened, reviewed, run,
};
use crate::workspace::Desktop;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::path::Path;
use tablecraft_display::{DateRepresentation, ScalarPresentation};
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fixed_scalars::{
    self as raw, Column, Scalar, batch,
};
use tablecraft_ui::{Cell, CellKind, FluentArgs};

const LARGE_INPUT: &str = "9007199254740995";

struct Case {
    original: [Option<Scalar>; 2],
    values: [Option<Scalar>; 2],
    sql: &'static str,
    singleton_sql: &'static str,
}

fn case(column: Column) -> Case {
    let (original, values, sql, singleton_sql) = match column {
        Column::ByteValue => (
            Scalar::Byte(250),
            [Scalar::Byte(251), Scalar::Byte(1)],
            "UPDATE Measurements SET ByteValue=CByte(Nz(ByteValue,0)+1)",
            "UPDATE Measurements SET ByteValue=CByte(251) WHERE ID=1",
        ),
        Column::ShortValue => (
            Scalar::Int16(-10),
            [Scalar::Int16(-8), Scalar::Int16(2)],
            "UPDATE Measurements SET ShortValue=CInt(Nz(ShortValue,0)+2)",
            "UPDATE Measurements SET ShortValue=CInt(-8) WHERE ID=1",
        ),
        Column::LargeValue => (
            Scalar::Int64(9_007_199_254_740_993),
            [Scalar::Int64(9_007_199_254_740_995), Scalar::Int64(1)],
            "PARAMETERS [Large] BIGINT; UPDATE Measurements SET LargeValue=IIf(ID=1,[Large],CLngLng(1))",
            "PARAMETERS [Large] BIGINT; UPDATE Measurements SET LargeValue=[Large] WHERE ID=1",
        ),
        Column::SingleValue => (
            Scalar::SingleBits(0xc020_0000),
            [
                Scalar::SingleBits(0xbfa0_0000),
                Scalar::SingleBits(0x3fa0_0000),
            ],
            "UPDATE Measurements SET SingleValue=CSng(Nz(SingleValue,0)+1.25)",
            "UPDATE Measurements SET SingleValue=CSng(-1.25) WHERE ID=1",
        ),
        Column::DoubleValue => (
            Scalar::DoubleBits(0xc004_0000_0000_0000),
            [
                Scalar::DoubleBits(0xbff4_0000_0000_0000),
                Scalar::DoubleBits(0x3ff4_0000_0000_0000),
            ],
            "UPDATE Measurements SET DoubleValue=CDbl(Nz(DoubleValue,0)+1.25)",
            "UPDATE Measurements SET DoubleValue=CDbl(-1.25) WHERE ID=1",
        ),
        Column::DateValue => (
            Scalar::DateTimeBits(0xc004_0000_0000_0000),
            [
                Scalar::DateTimeBits(0xbff8_0000_0000_0000),
                Scalar::DateTimeBits(0),
            ],
            "UPDATE Measurements SET DateValue=DateAdd('d',1,Nz(DateValue,#1899-12-29#))",
            "UPDATE Measurements SET DateValue=DateAdd('d',1,#1899-12-28T12:00:00#) WHERE ID=1",
        ),
    };

    Case {
        original: [Some(original), None],
        values: values.map(Some),
        sql,
        singleton_sql,
    }
}

fn observe(cell: &Cell, expected: Option<Scalar>) {
    let CellKind::Scalar { is_null, .. } = cell.kind else {
        panic!("native scalar observation");
    };
    assert_eq!(is_null, expected.is_none());

    match expected {
        None => assert_eq!(cell.text, "NULL"),
        Some(Scalar::Byte(value)) => assert_eq!(cell.text, value.to_string()),
        Some(Scalar::Int16(value)) => assert_eq!(cell.text, value.to_string()),
        Some(Scalar::Int64(value)) => assert_eq!(cell.text, value.to_string()),
        Some(Scalar::SingleBits(bits)) => assert_eq!(cell.text, f32::from_bits(bits).to_string()),
        Some(Scalar::DoubleBits(bits)) => assert_eq!(cell.text, f64::from_bits(bits).to_string()),
        Some(Scalar::DateTimeBits(bits)) => assert!(matches!(
            cell.presentation,
            Some(ScalarPresentation::DateTime {
                representation: DateRepresentation::LegacyBits(actual),
                ..
            }) if actual == bits
        )),
    }
}

fn review(harness: &mut Harness<'_, Desktop>, sql: &str, input: Option<&str>) -> u64 {
    let Some(input) = input else {
        return reviewed(harness, sql);
    };
    run(harness, sql);
    assert!(harness.state().query_parameters.is_some());
    assert!(harness.state().editing.query_action_review.is_none());
    assert!(!harness.state().editing.has_save());
    harness
        .get_by_role_and_label(Role::TextInput, "Large")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.event(egui::Event::Text(input.into()));
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(2);
    finish_reads(harness);
    assert!(harness.state().query_parameters.is_none());
    let (id, _) = harness
        .state()
        .editing
        .query_action_review
        .as_ref()
        .expect("confirmed exact BIGINT input issues a native review");
    *id
}

fn apply(
    harness: &mut Harness<'_, Desktop>,
    sql: &str,
    input: Option<&str>,
    before: &[u8],
    after: &[u8],
    path: &Path,
    (matched, changed): (usize, usize),
) {
    let original = harness.state().state.active_object().unwrap();
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let discarded = review(harness, sql, input);
    for (key, count) in [
        ("query-action-matched", matched),
        ("query-action-update-changed", changed),
    ] {
        let mut arguments = FluentArgs::new();
        arguments.set("count", count.to_string());
        let label = harness.state().state.localizer.format(key, &arguments);
        harness.get_by_role_and_label(Role::Label, &label);
    }
    assert_image(&std::fs::read(path).unwrap(), before);
    assert!(!journal_path(path).exists());
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
    assert_image(&std::fs::read(path).unwrap(), before);

    review(harness, sql, input);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("complete fixed-scalar save: {report:?}");
    };
    assert_eq!(
        (receipt.matched_rows, receipt.changed_rows),
        (Some(matched), Some(changed))
    );
    assert!(receipt.schema_change.is_none());
    assert_image(&std::fs::read(path).unwrap(), after);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
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
    assert!(!journal_path(path).exists());
}

fn run_case(column: Column) {
    let case = case(column);
    let source = batch::values(column, &case.original, &case.values);
    let (_directory, path, before, mut harness) = opened(source.before);
    let input = matches!(column, Column::LargeValue).then_some(LARGE_INPUT);
    apply(
        &mut harness,
        case.sql,
        input,
        &before,
        &source.expected,
        &path,
        (2, 2),
    );
    run(
        &mut harness,
        &format!("SELECT [{}] FROM Measurements ORDER BY ID", column.name()),
    );
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 2);
    for (row, value) in rows.iter().zip(case.values) {
        observe(&row[0], value);
    }
    assert_image(&std::fs::read(path).unwrap(), &source.expected);
}

#[test]
fn byte_batch_run_no_yes_saves_exact_bytes_and_reopens_fresh_rows() {
    run_case(Column::ByteValue);
}

#[test]
fn int16_batch_run_no_yes_saves_exact_bytes_and_reopens_fresh_rows() {
    run_case(Column::ShortValue);
}

#[test]
fn int64_batch_prompt_preserves_odd_integers_beyond_double_precision() {
    run_case(Column::LargeValue);
}

#[test]
fn single_batch_run_no_yes_saves_exact_finite_bits() {
    run_case(Column::SingleValue);
}

#[test]
fn double_batch_run_no_yes_saves_exact_finite_bits() {
    run_case(Column::DoubleValue);
}

#[test]
fn legacy_date_batch_run_no_yes_saves_exact_ole_bits() {
    run_case(Column::DateValue);
}

#[test]
fn actual_all_null_then_empty_and_old_singleton_keep_declared_byte_identity() {
    let column = Column::ByteValue;
    let case = case(column);
    let source = batch::values(column, &case.original, &case.original);
    let (_directory, path, mut before, mut harness) = opened(source.before);
    for (sql, values, counts) in [
        (
            "UPDATE Measurements SET ByteValue=NULL",
            [None, None],
            (2, 1),
        ),
        (
            "UPDATE Measurements SET ByteValue=NULL WHERE ID<0",
            [None, None],
            (0, 0),
        ),
        (case.singleton_sql, [Some(Scalar::Byte(251)), None], (1, 1)),
    ] {
        let expected = raw::update(&before, source.definition, 0, column, values[0]);
        let expected = raw::update(&expected, source.definition, 1, column, values[1]);
        apply(&mut harness, sql, None, &before, &expected, &path, counts);
        run(
            &mut harness,
            "SELECT ByteValue FROM Measurements ORDER BY ID",
        );
        let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
        assert_eq!(rows.len(), 2);
        for (row, value) in rows.iter().zip(values) {
            observe(&row[0], value);
        }
        before = expected;
    }
    assert_image(&std::fs::read(path).unwrap(), &before);
}

#[test]
fn late_byte_overflow_keeps_prior_results_and_every_original_companion() {
    let source = batch::values(
        Column::ByteValue,
        &[Some(Scalar::Byte(17)), None],
        &[Some(Scalar::Byte(17)), None],
    );
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
    let sql = "UPDATE Measurements SET ByteValue=CByte(IIf(ID=1,1,256))";
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
