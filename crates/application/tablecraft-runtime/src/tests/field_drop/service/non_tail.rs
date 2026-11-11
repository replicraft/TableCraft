//! Real middle-field DROP binds inspection, consuming save and survivor publication.
use super::super::inspect;
use super::{opened, request, review, terminal};
use crate::{
    Command, Error, Response, SaveOutcome, SaveSchemaChange, SaveStartFailure, SaveWorker,
};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, fields, typed_fields,
};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

fn journey(case: fields::Case, column: &str) {
    let source = fields::source(case);
    let before = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Amount",
        typed_fields::Kind::Currency,
    );
    let before = if matches!(case, fields::Case::FirstNumber) {
        typed_fields::update_currency(&before, source.definition, 0, Some(12345))
    } else {
        before
    };
    let after = field_changes::drop_field(&before, source.definition, source.catalog_root, column);
    let (_directory, path, worker, revision) = opened(&before);
    let page = |table: &str| {
        let Response::Data {
            source_revision,
            columns,
            rows,
            has_more,
            ..
        } = request(
            &worker,
            Command::Page {
                table: table.into(),
                offset: 0,
            },
        )
        .unwrap()
        else {
            panic!("expected complete native table page");
        };
        assert!(!has_more);
        (
            source_revision.unwrap(),
            columns
                .into_iter()
                .map(|field| (field.name, field.native_number))
                .collect::<Vec<_>>(),
            rows.into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|cell| (cell.text, cell.kind))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )
    };
    let (_, original_columns, original_rows) = page("Table1");
    assert_eq!(original_rows.len(), 2);
    assert_eq!(original_columns[1].0, column);
    assert_eq!(original_columns[1].1, Some(1));
    let (_, sibling_columns, sibling_rows) = page("Other");
    let facts = inspect(&worker, revision, column, 1).unwrap();
    assert!(
        facts.source_eligible,
        "actual middle target must have an eligible checked source"
    );
    assert_image(&std::fs::read(&path).unwrap(), &before);
    let (_, capability) = review(&worker, revision, column, 1).unwrap();
    let (_, stale) = review(&worker, revision, column, 1).unwrap();
    let witness = capability.reopen_witness();
    assert_image(witness.expected_bytes(), &after);
    assert_image(&std::fs::read(&path).unwrap(), &before);
    let saves = SaveWorker::new(|| {});
    let bundle = crate::journal_path(&path);
    let id = saves.start_field_drop(capability, bundle.clone()).unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, id);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.schema_change == Some(SaveSchemaChange::ColumnDropped) && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()),
        "{report:?}"
    );
    assert_eq!(
        page("Table1").0,
        revision,
        "terminal save does not publish a read session"
    );
    assert!(saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_image(&std::fs::read(&path).unwrap(), &after);
    let Response::Opened {
        document_revision: fresh,
        ..
    } = request(&worker, Command::OpenFieldDropped { witness }).unwrap()
    else {
        panic!("expected complete non-tail DROP witness reopening");
    };
    assert!(fresh > revision);
    assert!(matches!(
        inspect(&worker, revision, column, 1),
        Err(Error::StaleDocument)
    ));
    let failure = saves
        .start_field_drop(stale, crate::journal_path(&path))
        .unwrap_err();
    assert_eq!(failure.kind, SaveStartFailure::StaleDocument);
    assert!(!bundle.exists());
    let (current, columns, rows) = page("Table1");
    assert_eq!(current, fresh);
    assert_eq!(
        columns,
        original_columns
            .into_iter()
            .enumerate()
            .filter(|(index, _)| *index != 1)
            .map(|(_, column)| column)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        rows,
        original_rows
            .into_iter()
            .map(|row| row
                .into_iter()
                .enumerate()
                .filter(|(index, _)| *index != 1)
                .map(|(_, cell)| cell)
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    let (_, columns, rows) = page("Other");
    assert_eq!(columns, sibling_columns);
    assert_eq!(rows, sibling_rows);
    assert_image(&std::fs::read(&path).unwrap(), &after);
}

#[test]
fn populated_middle_number_drop_saves_acknowledges_and_reopens_exact_survivors() {
    journey(fields::Case::FirstNumber, "Score");
}

#[test]
fn populated_middle_text_drop_preserves_forwarded_rows_and_native_number_hole() {
    journey(fields::Case::SecondText, "Name");
}
