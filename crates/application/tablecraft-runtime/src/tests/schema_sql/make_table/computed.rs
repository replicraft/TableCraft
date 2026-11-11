//! Real consuming computed make-table review keeps native companions and no defaults.
use super::{assert_image, fields, inspect, populated, review};
use crate::tests::{request, stored::opened, terminal};
use crate::{
    Command, InsertInput, InsertTarget, Response, SaveOutcome, SaveSchemaChange, SaveWorker,
};
use std::{fs, io::Cursor};
use tablecraft_ace::{inspect_table_properties, inspect_table_schema};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::populated::computed::{
    self as raw, Row,
};

fn budget() -> tablecraft_core::limits::ResourceBudget {
    tablecraft_core::limits::ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}

#[test]
fn computed_table_keeps_native_text_decimal_sizes_and_fresh_omission_is_not_the_expression() {
    let mut seed = fields::source(fields::Case::FirstNumber).bytes;
    for (index, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        seed[110 + index] = [9, 4, 0, 1][index] ^ mask;
    }
    let coefficient = 90071992547409931234_i128;
    let before = populated::text_decimals(
        &seed,
        "Items",
        &[(Some("é🦀"), Some(coefficient)), (None, None)],
    );
    let rows = [
        Row {
            label: Some("é🦀"),
            amount: Some(coefficient),
            score: Some(1),
        },
        Row {
            label: None,
            amount: None,
            score: Some(1),
        },
    ];
    let expected = raw::after(&before, "Archive", &rows);
    let (_directory, path, worker, revision) = opened(&before);
    let sql = "SELECT i.Label,i.Amount,CLng(1) AS CopiedScore INTO Archive FROM Items AS i";
    let target = inspect(&worker, revision, sql);
    let Response::TableCreationReviewed { capability } = review(&worker, &target).unwrap() else {
        panic!("actual computed review");
    };
    assert_image(&fs::read(&path).unwrap(), &before);
    assert!(!crate::journal_path(&path).exists());
    drop(capability);
    let Response::TableCreationReviewed { capability } = review(&worker, &target).unwrap() else {
        panic!("fresh accepted computed review");
    };
    let witness = capability.reopen_witness();
    assert_eq!(witness.inserted_rows(), Some(2));
    assert_eq!(witness.column_count(), 3);
    assert_eq!(witness.draft_revision(), None);
    assert_image(witness.expected_bytes(), &expected);
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_table_creation(capability, crate::journal_path(&path))
        .unwrap();
    let report = terminal(&saves);
    assert!(matches!(report.outcome,SaveOutcome::Saved {receipt}
        if receipt.matched_rows.is_none() && receipt.changed_rows==Some(2)
            && receipt.schema_change==Some(SaveSchemaChange::TableAdded {column_count:3})));
    assert!(saves.acknowledge(id));
    let Response::Opened {
        document_revision: fresh,
        ..
    } = request(&worker, Command::OpenTableCreated { witness }).unwrap()
    else {
        panic!("guarded computed reopen");
    };
    assert!(fresh > revision);
    assert_image(&fs::read(&path).unwrap(), &expected);
    let actual = fs::read(&path).unwrap();
    let schema = inspect_table_schema(
        &mut Cursor::new(&actual),
        CatalogScope::new(19005),
        "Archive",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns()[0].storage_length(), 14);
    assert_eq!(schema.columns()[1].decimal_parameters(), Some((28, 4)));
    assert_eq!(schema.columns()[2].flags(), 3);
    assert_eq!(schema.physical_layout().physical_indexes, 0);
    let properties = inspect_table_properties(
        &mut Cursor::new(&actual),
        CatalogScope::new(19005),
        "Archive",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(properties.groups().is_empty());

    let Response::InsertDraft(draft) = request(
        &worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision: fresh,
                table: "Archive".into(),
            },
        },
    )
    .unwrap() else {
        panic!("actual manual created-table draft");
    };
    let Response::InsertReviewed { capability, .. } = request(
        &worker,
        Command::ReviewInsert {
            draft_id: draft.id,
            inputs: vec![
                InsertInput::Omitted,
                InsertInput::Omitted,
                InsertInput::Omitted,
            ],
        },
    )
    .unwrap() else {
        panic!("actual omission review");
    };
    let id = saves
        .start_insert(capability, crate::journal_path(&path))
        .unwrap();
    let report = terminal(&saves);
    assert!(matches!(report.outcome,SaveOutcome::Saved {receipt} if receipt.changed_rows==Some(1)));
    assert!(saves.acknowledge(id));
    let rows = [
        Row {
            label: Some("é🦀"),
            amount: Some(coefficient),
            score: Some(1),
        },
        Row {
            label: None,
            amount: None,
            score: Some(1),
        },
        Row {
            label: None,
            amount: None,
            score: None,
        },
    ];
    assert_image(
        &fs::read(&path).unwrap(),
        &raw::after(&before, "Archive", &rows),
    );
    assert!(!crate::journal_path(&path).exists());
}
