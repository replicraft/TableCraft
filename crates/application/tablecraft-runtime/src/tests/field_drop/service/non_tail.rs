//! Real middle-field DROP binds inspection, consuming save and survivor publication.
mod survivors;

use super::{opened, request, terminal};
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

fn source(case: fields::Case) -> fields::Source {
    let mut source = fields::source(case);
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
    source.bytes = before;
    source
}

struct Reopened {
    _directory: tempfile::TempDir,
    path: std::path::PathBuf,
    worker: crate::Worker,
    revision: u64,
    bytes: Vec<u8>,
    definition: u32,
}

fn journey(source: fields::Source, table: &str, column: &str) -> Reopened {
    let before = source.bytes;
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
    let (_, original_columns, original_rows) = page(table);
    assert_eq!(original_rows.len(), source.roots.len());
    let selected = original_columns
        .iter()
        .position(|field| field.0 == column)
        .unwrap();
    let number = original_columns[selected].1.unwrap();
    let inspect = |revision| {
        request(
            &worker,
            Command::InspectFieldDropSource {
                document_revision: revision,
                table: table.into(),
                column: column.into(),
                native_number: number,
            },
        )
        .map(|response| {
            let Response::FieldDropSourceInspected { inspection } = response else {
                panic!("expected checked source facts for the actual native field");
            };
            inspection
        })
    };
    let review = || {
        let Response::FieldDropReviewed { review, capability } = request(
            &worker,
            Command::ReviewFieldDrop {
                document_revision: revision,
                table: table.into(),
                column: column.into(),
                native_number: number,
            },
        )
        .unwrap() else {
            panic!("expected consuming non-tail DROP review");
        };
        assert_eq!(review.native_number, number);
        assert_eq!(capability.captured_native_number(), number);
        capability
    };
    let (_, sibling_columns, sibling_rows) = page("Other");
    let facts = inspect(revision).unwrap();
    assert!(
        facts.source_eligible,
        "actual middle target must have an eligible checked source"
    );
    assert_image(&std::fs::read(&path).unwrap(), &before);
    let capability = review();
    let stale = review();
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
        page(table).0,
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
    assert!(matches!(inspect(revision), Err(Error::StaleDocument)));
    let failure = saves
        .start_field_drop(stale, crate::journal_path(&path))
        .unwrap_err();
    assert_eq!(failure.kind, SaveStartFailure::StaleDocument);
    assert!(!bundle.exists());
    let (current, columns, rows) = page(table);
    assert_eq!(current, fresh);
    assert_eq!(
        columns,
        original_columns
            .into_iter()
            .enumerate()
            .filter(|(index, _)| *index != selected)
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
                .filter(|(index, _)| *index != selected)
                .map(|(_, cell)| cell)
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    let (_, columns, rows) = page("Other");
    assert_eq!(columns, sibling_columns);
    assert_eq!(rows, sibling_rows);
    assert_image(&std::fs::read(&path).unwrap(), &after);
    Reopened {
        _directory,
        path,
        worker,
        revision: fresh,
        bytes: after,
        definition: source.definition,
    }
}

#[test]
fn populated_middle_number_drop_saves_acknowledges_and_reopens_exact_survivors() {
    survivors::currency(source(fields::Case::FirstNumber));
}

#[test]
fn populated_middle_text_drop_preserves_forwarded_rows_and_native_number_hole() {
    journey(source(fields::Case::SecondText), "Table1", "Name");
}

#[test]
fn keyless_ordinary_native_zero_drop_preserves_score_one_without_primary_assumptions() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let catalog_root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let bytes = field_changes::keyless_first_field(&seed, "Keyless");
    survivors::keyless(fields::Source {
        bytes,
        definition,
        catalog_root,
        roots: vec![0],
    });
}
