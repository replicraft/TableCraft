//! Actual schema save, original Number inputs and native adapter save/reopen.
use super::{assert_retirement, begin, saved_draft, terminal, value};
use crate::tests::{open, request};
use crate::{
    CellKind, Command, Error, InsertDraft, InsertInput, Response, ReviewedInsert, SaveKnownOutcome,
    SaveOutcome, SaveWorker, Worker, journal_path,
};
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{NativeImage, inspect_table_properties, inspect_table_schema, read_table};
use tablecraft_builders::table::{DraftDefaultMode, DraftFieldType, DraftNumberSize, TableDraft};
use tablecraft_core::values::{Decimal, Double, Guid, Single, Value};
use tablecraft_schema::{DecimalConstraints, ScalarType};

const SIZES: [(DraftNumberSize, &str, ScalarType, u8); 7] = [
    (DraftNumberSize::Byte, "ByteValue", ScalarType::Byte, 2),
    (DraftNumberSize::Integer, "ShortValue", ScalarType::Int16, 3),
    (
        DraftNumberSize::LongInteger,
        "LongValue",
        ScalarType::Int32,
        4,
    ),
    (
        DraftNumberSize::Single,
        "SingleValue",
        ScalarType::Single,
        6,
    ),
    (
        DraftNumberSize::Double,
        "DoubleValue",
        ScalarType::Double,
        7,
    ),
    (
        DraftNumberSize::Decimal,
        "DecimalValue",
        ScalarType::Decimal,
        16,
    ),
    (
        DraftNumberSize::ReplicationId,
        "GuidValue",
        ScalarType::Guid,
        15,
    ),
];

fn number_draft(absent: bool) -> TableDraft {
    let mut draft = TableDraft::initial();
    for (size, name, _, _) in SIZES {
        let field = draft.add_field(DraftFieldType::Number).unwrap();
        draft.rename_field(field, name).unwrap();
        draft.set_field_number_size(field, size).unwrap();
        if size == DraftNumberSize::Decimal {
            draft
                .set_field_decimal_constraints(field, DecimalConstraints::new(6, 2).unwrap())
                .unwrap();
        }
        if absent {
            draft
                .set_field_default(field, DraftDefaultMode::Absent)
                .unwrap();
        }
    }
    draft
}

fn image(path: &Path) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(fs::read(path).unwrap()),
        crate::controls::SCOPE,
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap()
}

fn assert_schema(path: &Path, absent: bool) {
    let source = image(path);
    let schema = inspect_table_schema(
        &mut source.reader(),
        crate::controls::SCOPE,
        "Items",
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns().len(), 8);
    assert_eq!(schema.columns()[0].name(), "ID");
    for (field, (size, name, kind, tag)) in schema.columns()[1..].iter().zip(SIZES) {
        assert_eq!(field.name(), name);
        assert_eq!(field.scalar_type(), Some(kind));
        assert_eq!(field.native_type(), tag);
        assert_eq!(
            field.flags() & 0x44,
            0,
            "ordinary Number is not generated/replicated"
        );
        assert_eq!(
            field.decimal_parameters(),
            if size == DraftNumberSize::Decimal {
                Some((6, 2))
            } else {
                None
            }
        );
    }
    let properties = inspect_table_properties(
        &mut source.reader(),
        crate::controls::SCOPE,
        "Items",
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap();
    for (size, name, _, _) in SIZES {
        let default = properties
            .groups()
            .iter()
            .filter(|group| group.name() == name)
            .flat_map(|group| group.properties())
            .find(|property| property.name() == "DefaultValue");
        if absent || size == DraftNumberSize::ReplicationId {
            assert!(default.is_none(), "{name} must not invent a default");
        } else {
            let expected = match size {
                DraftNumberSize::Single | DraftNumberSize::Double => "0e0",
                DraftNumberSize::Decimal => "0.00",
                _ => "0",
            };
            assert_eq!(default.unwrap().text().unwrap(), expected);
        }
    }
}

fn reviewed(worker: &Worker, draft: &InsertDraft, inputs: Vec<InsertInput>) -> ReviewedInsert {
    let Response::InsertReviewed { review, capability } = request(
        worker,
        Command::ReviewInsert {
            draft_id: draft.id,
            inputs,
        },
    )
    .unwrap() else {
        panic!("expected a source-bound Number row review");
    };
    assert_eq!(review.inserted_rows, 1);
    assert!(matches!(review.inputs[0], InsertInput::Omitted));
    capability
}

fn save_and_reopen(
    worker: &Worker,
    saves: &SaveWorker,
    path: &Path,
    capability: ReviewedInsert,
) -> Vec<Value> {
    let operation = saves.start_insert(capability, journal_path(path)).unwrap();
    let report = terminal(saves);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("actual Number adapter save failed: {report:?}");
    };
    assert_eq!(report.id, operation);
    assert_eq!(receipt.known, SaveKnownOutcome::Committed);
    assert_eq!(receipt.changed_rows, Some(1));
    assert_eq!(receipt.schema_change, None);
    assert_retirement(receipt);
    assert!(saves.acknowledge(operation));
    assert!(!journal_path(path).exists());
    request(worker, open(path, "")).unwrap();
    let Response::Data { rows, columns, .. } = request(
        worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected actual reopened Number table contents");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(columns.len(), 8);
    assert_eq!(rows[0][0].text, "1");
    let native = read_table(
        &mut image(path).reader(),
        crate::controls::SCOPE,
        "Items",
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(native.rows().len(), 1);
    for (cell, stored) in rows[0].iter().zip(&native.rows()[0]) {
        assert!(
            matches!(cell.kind, CellKind::Scalar { is_null, .. } if is_null == matches!(stored, Value::Null))
        );
    }
    native.rows()[0].clone()
}

#[test]
fn all_number_sizes_save_exact_native_schema_and_omission_uses_only_declared_defaults() {
    for absent in [false, true] {
        let (_directory, path, worker, saves, revision) = saved_draft(number_draft(absent));
        assert_schema(&path, absent);
        let before = fs::read(&path).unwrap();
        let draft = begin(&worker, revision);
        assert_eq!(draft.fields.len(), 8);
        let inputs = (0..8).map(|_| InsertInput::Omitted).collect();
        let capability = reviewed(&worker, &draft, inputs);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());
        let row = save_and_reopen(&worker, &saves, &path, capability);
        let expected = if absent {
            vec![
                Value::Int32(1),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
            ]
        } else {
            vec![
                Value::Int32(1),
                Value::Byte(0),
                Value::Int16(0),
                Value::Int32(0),
                Value::Single(Single::from_bits(0)),
                Value::Double(Double::from_bits(0)),
                Value::Decimal(Decimal::new(0, 2).unwrap()),
                Value::Null,
            ]
        };
        assert_eq!(row, expected);
        assert_schema(&path, absent);
    }
}

fn supplied(decimal: &str) -> Vec<InsertInput> {
    vec![
        InsertInput::Omitted,
        value("255", false),
        value("-32768", false),
        value("2147483647", false),
        value("1e-45", false),
        value("-0", false),
        value(decimal, false),
        value("00112233-4455-6677-8899-aabbccddeeff", false),
    ]
}

#[test]
fn supplied_wide_number_row_uses_real_adapter_save_and_reopens_exact_carriers() {
    let (_directory, path, worker, saves, revision) = saved_draft(number_draft(false));
    let draft = begin(&worker, revision);
    let capability = reviewed(&worker, &draft, supplied("1234.50"));
    // Byte, Int16, Single, Double, Decimal and GUID cannot fit the legacy TCNA2
    // supplied grammar. This exercises the actual wider-intent adapter save path.
    let row = save_and_reopen(&worker, &saves, &path, capability);
    assert_eq!(
        row,
        vec![
            Value::Int32(1),
            Value::Byte(255),
            Value::Int16(i16::MIN),
            Value::Int32(i32::MAX),
            Value::Single(Single::from_bits(1)),
            Value::Double(Double::from_bits(0x8000_0000_0000_0000)),
            Value::Decimal(Decimal::new(123450, 2).unwrap()),
            Value::Guid(Guid::from_bytes([
                0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255
            ])),
        ]
    );
}

#[test]
fn nonexact_decimal_refuses_without_effect_and_same_draft_accepts_lossless_correction() {
    let (_directory, path, worker, saves, revision) = saved_draft(number_draft(false));
    let before = fs::read(&path).unwrap();
    let draft = begin(&worker, revision);
    for invalid in ["1.234", "10000.00"] {
        let error = match request(
            &worker,
            Command::ReviewInsert {
                draft_id: draft.id,
                inputs: supplied(invalid),
            },
        ) {
            Err(error) => error,
            Ok(_) => panic!("nonexact Decimal input was accepted: {invalid}"),
        };
        assert!(
            matches!(error, Error::InvalidScalarInput),
            "input {invalid}: expected insertion scalar-input refusal, got {error:?}",
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());
        assert!(!saves.is_busy());
    }
    let capability = reviewed(&worker, &draft, supplied("1234.5000"));
    assert_eq!(fs::read(&path).unwrap(), before);
    let row = save_and_reopen(&worker, &saves, &path, capability);
    assert_eq!(row[6], Value::Decimal(Decimal::new(123450, 2).unwrap()));
}
