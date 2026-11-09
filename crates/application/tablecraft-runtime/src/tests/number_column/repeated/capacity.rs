//! Raw modeled history beyond the second-ADD SDK pins exercises real runtime pools.
use super::*;

#[test]
fn last_fitting_short_number_uses_normal_runtime_budgets_through_full_reopen() {
    let mut before = raw::repeated::source(raw::Source::IdOnly);
    for number in 2..=12 {
        before = raw::repeated::append(&before, &format!("N{number}"));
    }
    // Independent fixture framing: twelve retained bodies leave 596 bytes. A
    // short N13 body plus its slot fits; the next such body cannot fit. This is
    // raw-model capacity evidence, not an additional SDK or native Access run.
    let page = &before[10 * 4096..11 * 4096];
    assert_eq!(u16::from_le_bytes(page[12..14].try_into().unwrap()), 12);
    assert_eq!(u16::from_le_bytes(page[2..4].try_into().unwrap()), 596);
    let expected = raw::repeated::append(&before, "N13");
    let page = &expected[10 * 4096..11 * 4096];
    assert_eq!(u16::from_le_bytes(page[12..14].try_into().unwrap()), 13);
    assert_eq!(u16::from_le_bytes(page[2..4].try_into().unwrap()), 71);
    assert_eq!(expected.len(), before.len());

    // No caller substitutes budgets or eligibility. Availability, review and
    // SaveWorker staging each use the connected production native pool, while
    // full-image proof/persistence keep their independent existing allowances.
    let (directory, path, worker, revision) = opened(&before);
    let observed = request(
        &worker,
        Command::InspectNumberColumnAvailability {
            document_revision: revision,
            table: "Table1".into(),
        },
    )
    .expect("qualified capacity-bound history must fit the normal availability budget");
    let Response::NumberColumnAvailability { availability } = observed else {
        panic!("expected actual history-source availability");
    };
    assert!(availability.available);
    assert_eq!(availability.document_revision, revision);
    let (_, capability) = review(&worker, revision, "Table1", "N13");
    let witness = capability.reopen_witness();
    assert_eq!(witness.expected_bytes(), expected);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let bundle = directory.path().join("capacity-number.journal");
    assert!(!bundle.exists());
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_number_column(capability, bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(SaveSchemaChange::ColumnAdded)
            && receipt.matched_rows.is_none()
            && receipt.changed_rows.is_none()),
        "normal staging budget refused qualified history: {report:?}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(saves.acknowledge(id));
    assert!(!bundle.exists());
    let opened = request(&worker, Command::OpenColumnAdded { witness })
        .expect("the complete capacity-bound AFTER must pass its actual witness");
    let Response::Opened {
        document_revision, ..
    } = opened
    else {
        panic!("expected full-image checked schema reopen");
    };
    assert_ne!(document_revision, revision);
    let Response::Details {
        fields,
        declared_rows,
        ..
    } = request(
        &worker,
        Command::Describe {
            table: "Table1".into(),
        },
    )
    .unwrap()
    else {
        panic!("the saved capacity-bound schema must remain readable");
    };
    assert_eq!(declared_rows, 0);
    assert_eq!(fields.len(), 14);
    let field = fields.last().unwrap();
    assert_eq!(field.name, "N13");
    assert_eq!(field.native_type, 4);
    assert_eq!(field.reader_nullable, Some(true));
    assert_eq!(std::fs::read(&path).unwrap(), expected);
}
