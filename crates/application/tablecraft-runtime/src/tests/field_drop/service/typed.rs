//! Typed tail metadata removal preserves every original record and survivor identity.
use super::{opened, request, terminal};
use crate::{Command, EditInput, EditTarget, Response, SaveOutcome, SaveSchemaChange, SaveWorker};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    exact_values, field_changes, fields,
    fixed_scalars::{self, Scalar},
};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

fn drop_sequence(
    table: &str,
    mut expected: Vec<u8>,
    definition: u32,
    catalog_root: usize,
    tails: &[(&str, usize)],
) {
    let (_directory, path, worker, mut revision) = opened(&expected);
    let saves = SaveWorker::new(|| {});
    let Response::Data {
        columns: original_columns,
        rows: original_rows,
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
        panic!("expected the original typed populated source");
    };
    assert_eq!(original_rows.len(), 1);
    for &(column, number) in tails {
        let before = expected;
        expected = field_changes::drop_tail(&before, definition, catalog_root, column);
        let Response::FieldDropReviewed { review, capability } = request(
            &worker,
            Command::ReviewFieldDrop {
                document_revision: revision,
                table: table.into(),
                column: column.into(),
                native_number: number,
            },
        )
        .unwrap_or_else(|error| panic!("{table}.{column} DROP review: {error:?}")) else {
            panic!("expected an actual typed tail DROP review");
        };
        assert_eq!(review.column, column);
        assert_eq!(review.native_number, number);
        let witness = capability.reopen_witness();
        assert_image(witness.expected_bytes(), &expected);
        assert_image(&std::fs::read(&path).unwrap(), &before);
        let id = saves
            .start_field_drop(capability, crate::journal_path(&path))
            .unwrap();
        let report = terminal(&saves);
        assert_eq!(report.id, id);
        assert!(
            matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.schema_change == Some(SaveSchemaChange::ColumnDropped)
                && receipt.changed_rows.is_none() && receipt.matched_rows.is_none()),
            "{report:?}"
        );
        assert!(saves.acknowledge(id));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        assert!(!crate::journal_path(&path).exists());
        let Response::Opened {
            document_revision, ..
        } = request(&worker, Command::OpenFieldDropped { witness }).unwrap()
        else {
            panic!("expected exact typed DROP reopening");
        };
        assert!(document_revision > revision);
        revision = document_revision;
        let Response::Data {
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
            panic!("expected fresh typed survivors");
        };
        assert_eq!(columns.len(), number);
        for (actual, original) in columns.iter().zip(&original_columns) {
            assert_eq!(actual.name, original.name);
            assert_eq!(actual.native_number, original.native_number);
        }
        assert_eq!(rows.len(), 1);
        for (actual, original) in rows[0].iter().zip(&original_rows[0]) {
            assert_eq!(actual.text, original.text);
            assert_eq!(actual.kind, original.kind);
        }
        assert!(!has_more);
    }
    let Response::Opened {
        document_revision, ..
    } = request(
        &worker,
        Command::Open {
            path: path.clone(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected fresh post-DROP source");
    };
    let Response::EditDraft(edit) = request(
        &worker,
        Command::BeginEdit {
            target: EditTarget {
                document_revision,
                table: table.into(),
                column: "Score".into(),
                parent_ordinal: 0,
            },
        },
    )
    .unwrap() else {
        panic!("expected freshly qualified survivor edit");
    };
    let Response::EditReviewed { capability, .. } = request(
        &worker,
        Command::ReviewEdit {
            draft_id: edit.id,
            input: EditInput {
                text: "124".into(),
                is_null: false,
            },
        },
    )
    .unwrap() else {
        panic!("expected actual survivor edit review");
    };
    let before = expected;
    expected = field_changes::update_score(&before, definition, 0, Some(124));
    assert_image(&std::fs::read(&path).unwrap(), &before);
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
    let Response::Data { columns, rows, .. } = request(
        &worker,
        Command::Page {
            table: table.into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected complete final survivor source");
    };
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0].name, "ID");
    assert_eq!(columns[1].name, "Score");
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "124");
    assert_image(&std::fs::read(&path).unwrap(), &expected);
}

#[test]
fn populated_measurements_drop_six_typed_tails_preserving_all_record_bytes() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let empty = fixed_scalars::append_table(&seed, "Measurements");
    let before = fixed_scalars::append_values(
        &empty,
        definition,
        &[
            Scalar::Byte(17),
            Scalar::Int16(-1234),
            Scalar::Int64(9_007_199_254_740_993),
            Scalar::SingleBits(0x3fa0_0000),
            Scalar::DoubleBits(0x3ff3_c0ca_428c_59dd),
            Scalar::DateTimeBits(0x4004_0000_0000_0000),
        ],
    );
    drop_sequence(
        "Measurements",
        before,
        definition,
        root,
        &[
            ("DateValue", 7),
            ("DoubleValue", 6),
            ("SingleValue", 5),
            ("LargeValue", 4),
            ("ShortValue", 3),
            ("ByteValue", 2),
        ],
    );
}

#[test]
fn populated_exact_values_drop_decimal_then_guid_preserving_all_record_bytes() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let empty = exact_values::append_table(&seed, "ExactValues");
    let before = exact_values::append(
        &empty,
        definition,
        Some([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ]),
        Some(90_071_992_547_409_931_234),
    );
    drop_sequence(
        "ExactValues",
        before,
        definition,
        root,
        &[("Amount", 3), ("Token", 2)],
    );
}
