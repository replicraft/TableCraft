//! Actual typed ADD saves distinguish old values, edited values and fresh defaults.
use super::{inspect, opened, request, review, terminal};
use crate::{
    CellKind, Command, EditInput, EditTarget, InsertInput, InsertTarget, Response, SaveOutcome,
    SaveSchemaChange, SaveWorker, Worker,
};
use std::{num::NonZeroU8, path::Path};
use tablecraft_builders::table::{FieldAdditionDeclaration, TypedFieldAdditionDeclaration};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields, typed_fields as raw,
};

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

fn scalar(kind: raw::Kind) -> ScalarType {
    match kind {
        raw::Kind::Boolean => ScalarType::Boolean,
        raw::Kind::Byte => ScalarType::Byte,
        raw::Kind::Int16 => ScalarType::Int16,
        raw::Kind::Int32 => ScalarType::Int32,
        raw::Kind::Int64 => ScalarType::Int64,
        raw::Kind::Single => ScalarType::Single,
        raw::Kind::Double => ScalarType::Double,
        raw::Kind::Currency => ScalarType::Currency,
        raw::Kind::DateTime => ScalarType::DateTime,
        raw::Kind::Guid => ScalarType::Guid,
        raw::Kind::Decimal { .. } => ScalarType::Decimal,
        raw::Kind::ShortText { .. } => ScalarType::Text,
    }
}

fn declaration(kind: raw::Kind) -> FieldAdditionDeclaration {
    FieldAdditionDeclaration::Typed(match kind {
        raw::Kind::Decimal { precision, scale } => {
            TypedFieldAdditionDeclaration::decimal(precision, scale).unwrap()
        }
        raw::Kind::ShortText { units, compression } => TypedFieldAdditionDeclaration::short_text(
            NonZeroU8::new(u8::try_from(units).unwrap()).unwrap(),
            compression,
        ),
        _ => TypedFieldAdditionDeclaration::fixed(scalar(kind)).unwrap(),
    })
}

fn reopen(worker: &Worker, path: &Path) -> u64 {
    let Response::Opened {
        document_revision, ..
    } = request(
        worker,
        Command::Open {
            path: path.into(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected a fresh native read session");
    };
    document_revision
}

fn page(worker: &Worker) -> Vec<Vec<crate::Cell>> {
    let Response::Data {
        columns,
        rows,
        has_more,
        ..
    } = request(
        worker,
        Command::Page {
            table: "Table1".into(),
            offset: 0,
        },
    )
    .unwrap()
    else {
        panic!("expected native rows after guarded reopen");
    };
    assert!(!has_more);
    assert_eq!(columns.len(), 3);
    assert_eq!(columns[2].name, "AddedValue");
    assert_eq!(columns[2].native_number, Some(2));
    rows
}

fn add(
    worker: &Worker,
    saves: &SaveWorker,
    path: &Path,
    revision: u64,
    source: &fields::Source,
    kind: raw::Kind,
) -> (u64, Vec<u8>) {
    let facts = inspect(worker, revision);
    assert!(
        facts
            .typed_declarations
            .iter()
            .any(|fact| fact.declaration.descriptor().scalar_type() == scalar(kind))
    );
    let expected = raw::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "AddedValue",
        kind,
    );
    let (_, capability) = review(worker, revision, "AddedValue", declaration(kind));
    let witness = capability.reopen_witness();
    equal_image(witness.expected_bytes(), &expected);
    equal_image(&std::fs::read(path).unwrap(), &source.bytes);
    let id = saves
        .start_field_addition(capability, crate::journal_path(path))
        .unwrap();
    let report = terminal(saves);
    assert_eq!(report.id, id);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.schema_change == Some(SaveSchemaChange::ColumnAdded) && receipt.changed_rows.is_none()),
        "{report:?}"
    );
    assert!(saves.acknowledge(id));
    assert!(!crate::journal_path(path).exists());
    equal_image(&std::fs::read(path).unwrap(), &expected);
    let Response::Opened {
        document_revision, ..
    } = request(worker, Command::OpenColumnAdded { witness }).unwrap()
    else {
        panic!("expected declaration-bound full-image reopen");
    };
    assert_ne!(document_revision, revision);
    (document_revision, expected)
}

fn insert(
    worker: &Worker,
    saves: &SaveWorker,
    path: &Path,
    revision: u64,
    before: &[u8],
    definition: u32,
    kind: raw::Kind,
) -> Vec<u8> {
    let Response::InsertDraft(draft) = request(
        worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision: revision,
                table: "Table1".into(),
            },
        },
    )
    .unwrap() else {
        panic!("expected actual post-ADD insertion declarations");
    };
    assert_eq!(draft.fields.len(), 3);
    assert!(draft.fields[0].auto_number);
    assert_eq!(draft.fields[2].name, "AddedValue");
    let Response::InsertReviewed { capability, review } = request(
        worker,
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
        panic!("expected consuming omitted-default review");
    };
    assert_eq!(review.inserted_rows, 1);
    let expected = raw::append_omitted(before, definition, kind);
    equal_image(&std::fs::read(path).unwrap(), before);
    let id = saves
        .start_insert(capability, crate::journal_path(path))
        .unwrap();
    let report = terminal(saves);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(1) && receipt.schema_change.is_none()),
        "{report:?}"
    );
    assert!(saves.acknowledge(id));
    assert!(!crate::journal_path(path).exists());
    equal_image(&std::fs::read(path).unwrap(), &expected);
    reopen(worker, path);
    expected
}

#[test]
fn all_twelve_saved_field_kinds_review_save_ack_reopen_and_insert_exact_type_defaults() {
    for kind in raw::KINDS {
        let source = fields::source(fields::Case::FirstNumber);
        let (_directory, path, worker, revision) = opened(&source.bytes);
        let saves = SaveWorker::new(|| {});
        let (revision, added) = add(&worker, &saves, &path, revision, &source, kind);
        let old_rows = page(&worker);
        assert_eq!(old_rows.len(), 2);
        for row in &old_rows {
            assert!(
                matches!(row[2].kind, CellKind::Scalar { is_null, boolean, .. } if if matches!(kind, raw::Kind::Boolean) { !is_null && boolean == Some(false) } else { is_null && boolean.is_none() })
            );
        }
        let expected = insert(
            &worker,
            &saves,
            &path,
            revision,
            &added,
            source.definition,
            kind,
        );
        let rows = page(&worker);
        assert_eq!(rows.len(), old_rows.len() + 1);
        for (old, new) in old_rows.iter().zip(&rows) {
            for (old, new) in old.iter().zip(new) {
                assert_eq!(old.kind, new.kind);
                assert_eq!(old.text, new.text);
            }
        }
        let fresh = rows.last().unwrap().last().unwrap();
        let nullable_default = matches!(
            kind,
            raw::Kind::DateTime | raw::Kind::Guid | raw::Kind::ShortText { .. }
        );
        assert!(
            matches!(fresh.kind, CellKind::Scalar { is_null, boolean, .. } if is_null == nullable_default && boolean == matches!(kind, raw::Kind::Boolean).then_some(false))
        );
        equal_image(&std::fs::read(&path).unwrap(), &expected);
    }
}

#[test]
fn added_boolean_materializes_only_true_and_currency_preserves_null_then_fresh_defaults() {
    for kind in [raw::Kind::Boolean, raw::Kind::Currency] {
        let source = fields::source(fields::Case::FirstNumber);
        let (_directory, path, worker, revision) = opened(&source.bytes);
        let saves = SaveWorker::new(|| {});
        let (mut revision, mut expected) = add(&worker, &saves, &path, revision, &source, kind);
        let edits: &[(&str, bool)] = if matches!(kind, raw::Kind::Boolean) {
            &[
                ("false", false),
                ("true", false),
                ("false", false),
                ("true", false),
            ]
        } else {
            &[("1.2345", false), ("ignored", true), ("-2.0001", false)]
        };
        for (index, &(text, is_null)) in edits.iter().enumerate() {
            let Response::EditDraft(draft) = request(
                &worker,
                Command::BeginEdit {
                    target: EditTarget {
                        document_revision: revision,
                        table: "Table1".into(),
                        column: "AddedValue".into(),
                        parent_ordinal: 0,
                    },
                },
            )
            .unwrap() else {
                panic!("expected editable added declaration");
            };
            if index == 0 {
                assert!(
                    matches!(draft.original.kind, CellKind::Scalar { is_null, boolean, .. } if if matches!(kind, raw::Kind::Boolean) { !is_null && boolean == Some(false) } else { is_null })
                );
            }
            let Response::EditReviewed { capability, review } = request(
                &worker,
                Command::ReviewEdit {
                    draft_id: draft.id,
                    input: EditInput {
                        text: text.into(),
                        is_null,
                    },
                },
            )
            .unwrap() else {
                panic!("expected exact typed historical row review");
            };
            let after = if matches!(kind, raw::Kind::Boolean) {
                raw::update_boolean(&expected, source.definition, 0, text == "true")
            } else {
                raw::update_currency(
                    &expected,
                    source.definition,
                    0,
                    if is_null {
                        None
                    } else if index == 0 {
                        Some(12345)
                    } else {
                        Some(-20001)
                    },
                )
            };
            let changed = usize::from(after != expected);
            assert_eq!(review.changed_rows, changed);
            equal_image(&std::fs::read(&path).unwrap(), &expected);
            let id = saves.start(capability, crate::journal_path(&path)).unwrap();
            let report = terminal(&saves);
            assert!(
                matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(changed)),
                "{report:?}"
            );
            assert!(saves.acknowledge(id));
            assert!(!crate::journal_path(&path).exists());
            expected = after;
            equal_image(&std::fs::read(&path).unwrap(), &expected);
            revision = reopen(&worker, &path);
            assert_eq!(page(&worker).len(), 2);
        }
        let expected = insert(
            &worker,
            &saves,
            &path,
            revision,
            &expected,
            source.definition,
            kind,
        );
        let rows = page(&worker);
        assert_eq!(rows.len(), 3);
        assert!(
            matches!(rows.last().unwrap()[2].kind, CellKind::Scalar { is_null: false, boolean, .. } if boolean == matches!(kind, raw::Kind::Boolean).then_some(false))
        );
        equal_image(&std::fs::read(&path).unwrap(), &expected);
    }
}
