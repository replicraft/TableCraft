//! A native-number hole does not turn surviving stored cells into display positions.
use super::{Reopened, assert_image, journey};
use crate::tests::{request, terminal};
use crate::{
    Command, EditInput, EditTarget, InsertInput, InsertTarget, Response, SaveOutcome, SaveWorker,
};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, fields, typed_fields,
};

pub(super) fn currency(source: fields::Source) {
    // Metadata-only DROP commutes with this literal fixed-cell replacement.
    // The original three-column oracle stays closed: it is never given a
    // two-active-column source or asked to infer a retired declaration.
    let changed = typed_fields::update_currency(
        &source.bytes,
        source.definition,
        source.roots[0],
        Some(-98765),
    );
    let expected =
        field_changes::drop_field(&changed, source.definition, source.catalog_root, "Score");
    let mut owner = journey(source, "Table1", "Score");
    edit(&mut owner, "Table1", "Amount", 2, 1, "-9.8765", expected);
    let inserted = field_changes::inserting::currency(&owner.bytes, owner.definition, Some(0));
    omitted_insert(
        &mut owner,
        "Table1",
        &["ID", "Amount"],
        &[0, 2],
        inserted,
        &[&["1", "-9.8765"], &["2", "NULL"], &["3", "0.0000"]],
    );
}

pub(super) fn keyless(source: fields::Source) {
    let mut owner = journey(source, "Keyless", "Id");
    let expected = field_changes::update_score(&owner.bytes, owner.definition, 0, Some(124));
    edit(&mut owner, "Keyless", "Score", 1, 0, "124", expected);
    let inserted = field_changes::inserting::keyless_score(&owner.bytes, owner.definition, None);
    omitted_insert(
        &mut owner,
        "Keyless",
        &["Score"],
        &[1],
        inserted,
        &[&["124"], &["NULL"]],
    );
}

fn edit(
    owner: &mut Reopened,
    table: &str,
    column: &str,
    native_number: usize,
    display_position: usize,
    text: &str,
    expected: Vec<u8>,
) {
    let Response::Data {
        columns,
        rows,
        source_revision,
        ..
    } = request(
        &owner.worker,
        Command::Page {
            table: table.into(),
            offset: 0,
        },
    )
    .unwrap()
    else {
        panic!("expected fresh post-DROP survivor page");
    };
    assert_eq!(source_revision, Some(owner.revision));
    assert_eq!(columns[display_position].name, column);
    assert_eq!(columns[display_position].native_number, Some(native_number));
    assert!(!rows.is_empty());
    let Response::EditDraft(draft) = request(
        &owner.worker,
        Command::BeginEdit {
            target: EditTarget {
                document_revision: owner.revision,
                table: table.into(),
                column: column.into(),
                parent_ordinal: 0,
            },
        },
    )
    .unwrap() else {
        panic!("survivor must retain an actual edit draft");
    };
    let Response::EditReviewed { capability, review } = request(
        &owner.worker,
        Command::ReviewEdit {
            draft_id: draft.id,
            input: EditInput {
                text: text.into(),
                is_null: false,
            },
        },
    )
    .unwrap() else {
        panic!("survivor native field must qualify its real replacement");
    };
    assert_eq!(review.changed_rows, 1);
    assert_eq!(capability.target().column, column);
    assert_eq!(capability.target().parent_ordinal, 0);
    assert_image(&std::fs::read(&owner.path).unwrap(), &owner.bytes);
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start(capability, crate::journal_path(&owner.path))
        .unwrap();
    let report = terminal(&saves);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows == Some(1) && receipt.changed_rows == Some(1)
            && receipt.schema_change.is_none()),
        "{report:?}"
    );
    assert!(saves.acknowledge(id));
    assert_image(&std::fs::read(&owner.path).unwrap(), &expected);
    assert!(!crate::journal_path(&owner.path).exists());
    let Response::Opened {
        document_revision, ..
    } = request(
        &owner.worker,
        Command::Open {
            path: owner.path.clone(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected fresh survivor edit reopen");
    };
    assert!(document_revision > owner.revision);
    owner.revision = document_revision;
    owner.bytes = expected;
    let Response::Data { columns, rows, .. } = request(
        &owner.worker,
        Command::Page {
            table: table.into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected freshly decoded survivor replacement");
    };
    assert_eq!(columns[display_position].native_number, Some(native_number));
    assert_eq!(columns[display_position].name, column);
    assert_eq!(rows[0][display_position].text, text);
}

fn omitted_insert(
    owner: &mut Reopened,
    table: &str,
    fields: &[&str],
    numbers: &[usize],
    expected: Vec<u8>,
    expected_rows: &[&[&str]],
) {
    let Response::InsertDraft(draft) = request(
        &owner.worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision: owner.revision,
                table: table.into(),
            },
        },
    )
    .unwrap() else {
        panic!("expected survivor insertion metadata");
    };
    assert_eq!(
        draft
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        fields
    );
    let Response::InsertReviewed { review, capability } = request(
        &owner.worker,
        Command::ReviewInsert {
            draft_id: draft.id,
            inputs: fields.iter().map(|_| InsertInput::Omitted).collect(),
        },
    )
    .unwrap_or_else(|error| panic!("post-DROP {table} omitted insertion review refused: {error}")) else {
        panic!("expected actual source-qualified omitted insertion review");
    };
    assert_eq!(review.inserted_rows, 1);
    assert!(
        review
            .inputs
            .iter()
            .all(|input| matches!(input, InsertInput::Omitted))
    );
    assert_eq!(capability.target().table, table);
    assert_image(&std::fs::read(&owner.path).unwrap(), &owner.bytes);
    commit_insert(owner, capability, expected);
    let Response::Data {
        columns,
        rows,
        source_revision,
        has_more,
        ..
    } = request(
        &owner.worker,
        Command::Page {
            table: table.into(),
            offset: 0,
        },
    )
    .unwrap()
    else {
        panic!("expected fresh complete post-DROP insertion page");
    };
    assert_eq!(source_revision, Some(owner.revision));
    assert!(!has_more);
    assert_eq!(
        columns
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        fields
    );
    assert_eq!(
        columns
            .iter()
            .map(|field| field.native_number)
            .collect::<Vec<_>>(),
        numbers.iter().copied().map(Some).collect::<Vec<_>>()
    );
    assert_eq!(
        rows.iter()
            .map(|row| row
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        expected_rows
    );
}

fn commit_insert(owner: &mut Reopened, capability: crate::ReviewedInsert, expected: Vec<u8>) {
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_insert(capability, crate::journal_path(&owner.path))
        .unwrap();
    let report = terminal(&saves);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.changed_rows == Some(1) && receipt.schema_change.is_none()),
        "{report:?}"
    );
    assert!(saves.acknowledge(id));
    assert_image(&std::fs::read(&owner.path).unwrap(), &expected);
    assert!(!crate::journal_path(&owner.path).exists());
    let Response::Opened {
        document_revision, ..
    } = request(
        &owner.worker,
        Command::Open {
            path: owner.path.clone(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected fresh insertion source after acknowledgement");
    };
    assert!(document_revision > owner.revision);
    owner.revision = document_revision;
    owner.bytes = expected;
}
