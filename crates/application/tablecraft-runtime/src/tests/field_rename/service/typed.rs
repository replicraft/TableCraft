//! Typed property groups retain their values through schema and later cell saves.
use super::{opened, request, terminal};
use crate::{
    Command, EditError, EditInput, EditTarget, Error, Response, SaveOutcome, SaveSchemaChange,
    SaveStartFailure, SaveWorker,
};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    exact_values, field_changes, fields,
    fixed_scalars::{self, Column, Scalar},
};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn typed_byte_guid_and_decimal_rename_then_edit_preserve_native_values_and_ordinals() {
    let token = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];
    for original in ["ByteValue", "Token", "Amount"] {
        let seed = fields::source(fields::Case::FirstNumber).bytes;
        let definition = (seed.len() / 4096) as u32;
        let catalog_root = usize::from(u16::from_le_bytes(
            seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
        ));
        let table = if original == "ByteValue" {
            "Measurements"
        } else {
            "ExactValues"
        };
        let mut expected = if original == "ByteValue" {
            let empty = fixed_scalars::append_table(&seed, table);
            fixed_scalars::append(&empty, definition)
        } else {
            let empty = exact_values::append_table(&seed, table);
            exact_values::append(
                &empty,
                definition,
                Some(token),
                Some(90_071_992_547_409_931_234),
            )
        };
        let (_directory, path, worker, mut revision) = opened(&expected);
        let saves = SaveWorker::new(|| {});
        let number = if original == "Amount" { 3 } else { 2 };
        for (old, native_number, new) in [
            ("Score", 1, "Reading"),
            (original, number, "ExpandedTypedValue"),
            ("ExpandedTypedValue", number, "X"),
            ("X", number, "x"),
            ("x", number, "x"),
        ] {
            let before = expected;
            expected = field_changes::rename(&before, definition, catalog_root, old, new);
            let Response::FieldRenameReviewed { review, capability } = request(
                &worker,
                Command::ReviewFieldRename {
                    document_revision: revision,
                    table: table.into(),
                    column: old.into(),
                    native_number,
                    new_name: new.into(),
                },
            )
            .unwrap_or_else(|error| panic!("{table}.{old} -> {new}: {error:?}")) else {
                panic!("expected actual typed rename review");
            };
            assert_eq!(review.changed, old != new);
            assert_image(capability.reopen_witness().expected_bytes(), &expected);
            assert_image(&std::fs::read(&path).unwrap(), &before);
            let witness = if old == new {
                let refused = saves
                    .start_field_rename(capability, crate::journal_path(&path))
                    .unwrap_err();
                assert_eq!(refused.kind, SaveStartFailure::InvalidState);
                refused
                    .into_reviewed()
                    .unwrap()
                    .into_unchanged_reopen()
                    .unwrap_or_else(|_| panic!("expected unchanged read witness"))
            } else {
                let witness = capability.reopen_witness();
                let id = saves
                    .start_field_rename(capability, crate::journal_path(&path))
                    .unwrap();
                let report = terminal(&saves);
                assert_eq!(report.id, id);
                assert!(
                    matches!(report.outcome, SaveOutcome::Saved { receipt }
                    if receipt.schema_change == Some(SaveSchemaChange::ColumnRenamed)
                        && receipt.changed_rows.is_none() && receipt.matched_rows.is_none()),
                    "{report:?}"
                );
                assert!(saves.acknowledge(id));
                witness
            };
            assert_image(&std::fs::read(&path).unwrap(), &expected);
            let Response::Opened {
                document_revision, ..
            } = request(&worker, Command::OpenFieldRenamed { witness }).unwrap()
            else {
                panic!("expected checked renamed source reopening");
            };
            revision = document_revision;
            let Response::Data { columns, rows, .. } = request(
                &worker,
                Command::Page {
                    table: table.into(),
                    offset: 0,
                },
            )
            .unwrap() else {
                panic!("expected renamed typed table");
            };
            assert_eq!(columns[native_number].name, new);
            assert_eq!(columns[native_number].native_number, Some(native_number));
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0][0].text, "1");
        }
        assert!(matches!(
            request(
                &worker,
                Command::BeginEdit {
                    target: EditTarget {
                        document_revision: revision,
                        table: table.into(),
                        column: original.into(),
                        parent_ordinal: 0,
                    }
                }
            ),
            Err(Error::Edit(EditError::TargetMissing))
        ));
        let Response::EditDraft(edit) = request(
            &worker,
            Command::BeginEdit {
                target: EditTarget {
                    document_revision: revision,
                    table: table.into(),
                    column: "x".into(),
                    parent_ordinal: 0,
                },
            },
        )
        .unwrap() else {
            panic!("expected typed edit by renamed field");
        };
        let text = match original {
            "ByteValue" => "255",
            "Token" => "42424242-4242-4242-4242-424242424242",
            "Amount" => "-9007199254740993.4321",
            _ => unreachable!(),
        };
        let Response::EditReviewed { capability, .. } = request(
            &worker,
            Command::ReviewEdit {
                draft_id: edit.id,
                input: EditInput {
                    text: text.into(),
                    is_null: false,
                },
            },
        )
        .unwrap() else {
            panic!("expected existing typed service after rename");
        };
        expected = match original {
            "ByteValue" => fixed_scalars::update(
                &expected,
                definition,
                0,
                Column::ByteValue,
                Some(Scalar::Byte(255)),
            ),
            "Token" => exact_values::update_guid(&expected, definition, 0, Some([0x42; 16])),
            "Amount" => exact_values::update_decimal(
                &expected,
                definition,
                0,
                Some(-90_071_992_547_409_934_321),
            ),
            _ => unreachable!(),
        };
        let id = saves.start(capability, crate::journal_path(&path)).unwrap();
        let report = terminal(&saves);
        assert!(
            matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.changed_rows == Some(1) && receipt.schema_change.is_none()),
            "{report:?}"
        );
        assert!(saves.acknowledge(id));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        assert!(matches!(
            request(
                &worker,
                Command::Open {
                    path: path.clone(),
                    password: String::new()
                }
            ),
            Ok(Response::Opened { .. })
        ));
        assert!(!crate::journal_path(&path).exists());
    }
}
