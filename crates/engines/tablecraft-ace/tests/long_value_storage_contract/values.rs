use super::model::{self, DECLARATION};
use tablecraft_ace::{InspectError, LongValueCheckStatus};
use tablecraft_core::values::Value;

#[test]
fn reachable_chunks_and_uninterpreted_owned_orphans_are_distinct_facts() {
    let bytes = model::image(1, true);
    assert_eq!(model::read(&bytes)[0][1], Value::Text("A".into()));
    let report = model::check(&bytes).unwrap();
    assert_eq!(report.rows(), 1);
    assert_eq!(report.columns().len(), 1);
    let column = &report.columns()[0];
    assert_eq!(column.status(), LongValueCheckStatus::Checked);
    assert_eq!(column.column_number(), 1);
    assert_eq!(column.maps().unwrap().owned(), 0x0b02);
    assert_eq!(column.maps().unwrap().candidate(), 0x0b03);
    assert_eq!((column.owned_pages(), column.candidate_pages()), (1, 1));
    assert_eq!(
        (
            column.owned_rows(),
            column.distinct_chunks(),
            column.orphan_rows()
        ),
        (2, 1, 1)
    );
    assert_eq!(column.values().single(), 1);
    assert_eq!(column.shared_references(), 0);
}

#[test]
fn null_keeps_preexisting_owned_rows_as_valid_orphans() {
    let bytes = model::image(1, false);
    assert_eq!(model::read(&bytes)[0][1], Value::Null);
    let report = model::check(&bytes).unwrap();
    let column = &report.columns()[0];
    assert_eq!(column.status(), LongValueCheckStatus::Checked);
    assert_eq!(column.values().nulls(), 1);
    assert_eq!(
        (
            column.owned_rows(),
            column.distinct_chunks(),
            column.orphan_rows()
        ),
        (2, 0, 2)
    );
}

#[test]
fn shared_live_references_are_reported_without_invented_exclusivity() {
    let bytes = model::image(2, true);
    let rows = model::read(&bytes);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row[1] == Value::Text("A".into())));
    let report = model::check(&bytes).unwrap();
    let column = &report.columns()[0];
    assert_eq!(column.status(), LongValueCheckStatus::Checked);
    assert_eq!(column.values().single(), 2);
    assert_eq!(column.distinct_chunks(), 1);
    assert_eq!(column.shared_references(), 1);
    assert_eq!(column.orphan_rows(), 1);
}

#[test]
fn overlapping_column_owned_pages_are_visible_without_invented_corruption() {
    let bytes = model::overlapping_columns_image();
    let rows = model::read(&bytes);
    assert_eq!(rows[0][1], Value::Text("A".into()));
    assert_eq!(rows[0][2], Value::Binary(vec![0x41, 0]));
    let report = model::check(&bytes).unwrap();
    assert_eq!(report.columns().len(), 2);
    for column in report.columns() {
        assert_eq!(column.status(), LongValueCheckStatus::Checked);
        assert_eq!(column.shared_owned_pages(), 1);
        assert_eq!(column.distinct_chunks(), 1);
        assert_eq!(column.orphan_rows(), 1);
    }
}

#[test]
fn ordinary_reads_do_not_establish_column_or_global_ownership() {
    for missing_column in [true, false] {
        let mut bytes = model::image(1, true);
        if missing_column {
            model::set_member(&mut bytes, 11, 2, 13, false);
            model::set_member(&mut bytes, 11, 3, 13, false);
        } else {
            model::set_member(&mut bytes, 1, 0, 13, true);
        }
        assert_eq!(model::read(&bytes)[0][1], Value::Text("A".into()));
        assert!(matches!(model::check(&bytes), Err(InspectError::Malformed)));
    }
}

#[test]
fn absent_and_historical_declarations_remain_explicit_without_narrowing_reads() {
    let mut absent = model::image(1, true);
    absent[DECLARATION..DECLARATION + 2].copy_from_slice(&[255, 255]);
    assert_eq!(model::read(&absent)[0][1], Value::Text("A".into()));
    let report = model::check(&absent).unwrap();
    assert_eq!(report.columns()[0].status(), LongValueCheckStatus::Absent);

    let mut historical = model::image(1, true);
    historical[DECLARATION..DECLARATION + 2].copy_from_slice(&[99, 0]);
    assert_eq!(model::read(&historical)[0][1], Value::Text("A".into()));
    let report = model::check(&historical).unwrap();
    assert!(report.columns().iter().any(|column| {
        column.column_number() == 99
            && column.status() == LongValueCheckStatus::HistoricalDeclaration
    }));
}
