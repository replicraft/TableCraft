//! Wide ordinary Number values retain complete original presence and raw AFTER.
use super::{Storage, budget, declarations, fixtures, image, scope};
use sha2::{Digest, Sha256};
use tablecraft_ace::{
    CreateColumn, EditError, InsertFieldValue, NativeOmittedInsertIntent, OmittedRowInsert,
    RecoverableEditError, WriteBlocker, create_ace16_database,
    qualify_native_omitted_insert_recovery, read_table,
};
use tablecraft_core::values::{Decimal, Double, Single, Value};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::{
    canonical_creation_properties::with_default_expressions, tcna2,
};

fn request(values: &[Value]) -> Vec<OmittedRowInsert> {
    let mut fields = vec![InsertFieldValue::Omitted];
    fields.extend(
        values
            .iter()
            .skip(1)
            .cloned()
            .map(InsertFieldValue::Supplied),
    );
    vec![OmittedRowInsert::new(fields)]
}

#[test]
fn wide_supplied_and_nullable_omissions_match_independent_entire_native_after() {
    for nulls in [false, true] {
        let (before, after, row) = fixtures::number_companions(nulls);
        assert_eq!(before, declarations::number_companions().bytes);
        let supplied = image(&before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                "Items",
                request(&row),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(supplied.after_bytes(), after);
        let mut hash = Sha256::new();
        hash.update(b"TCNI0001:BEFORE\0");
        hash.update((before.len() as u64).to_le_bytes());
        hash.update(&before);
        let mut original = vec![None];
        original.extend(row.iter().skip(1).cloned().map(Some));
        let expected = if nulls {
            tcna2::original_presence_wire(
                "Items",
                before.len(),
                hash.finalize().into(),
                vec![original],
            )
        } else {
            tcna2::wider_presence_wire(
                "Items",
                before.len(),
                hash.finalize().into(),
                vec![original],
            )
        };
        assert_eq!(
            supplied.intent().encode(&mut budget(), &|| false).unwrap(),
            expected
        );
        assert_eq!(
            read_table(
                &mut image(&after).reader(),
                scope(),
                "Items",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            &[row]
        );
        if nulls {
            let omitted = image(&before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    vec![OmittedRowInsert::new(
                        (0..7).map(|_| InsertFieldValue::Omitted).collect(),
                    )],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(omitted.after_bytes(), after);
            assert_eq!(
                &omitted.intent().encode(&mut budget(), &|| false).unwrap()[..8],
                b"TCNA0002"
            );
        }
    }
}

#[test]
fn wider_original_intent_replays_commit_and_interrupted_recovery() {
    let (before, after, row) = fixtures::number_companions(false);
    for committed in [false, true] {
        let plan = image(&before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                "Items",
                request(&row),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let raw = plan.intent().encode(&mut budget(), &|| false).unwrap();
        let mut target = Storage::new(before.clone());
        let mut journal = Storage::new(Vec::new());
        if !committed {
            target.fail_prefix = Some(8 * 4096 + 4090);
        }
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
        } else {
            assert!(prepared.commit(&mut budget(), &|| false).is_err());
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let original = NativeOmittedInsertIntent::decode(&raw, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_omitted_insert_recovery(
            checked,
            original,
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
            if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        assert_eq!(
            target.bytes.get_ref(),
            if committed { &after } else { &before }
        );
    }
}

#[test]
fn wider_companions_refuse_nonfinite_wrong_carriers_and_decimal_rounding() {
    let (before, _, row) = fixtures::number_companions(false);
    for (position, invalid) in [
        (1, Value::Int16(255)),
        (2, Value::Int32(-32768)),
        (3, Value::Single(Single::from_bits(0x7f80_0000))),
        (3, Value::Single(Single::from_bits(0x7fc0_0000))),
        (4, Value::Double(Double::from_bits(0xfff0_0000_0000_0000))),
        (4, Value::Single(Single::from_bits(1))),
        (5, Value::Binary(vec![0; 16])),
        (6, Value::Decimal(Decimal::new(-12345, 3).unwrap())),
        (
            6,
            Value::Decimal(Decimal::new(1_000_000_000_000_000_000, 4).unwrap()),
        ),
    ] {
        let mut changed = row.clone();
        changed[position] = invalid;
        assert!(
            matches!(
                image(&before).plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    request(&changed),
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            ),
            "field {position}"
        );
    }
}

#[test]
fn wider_constant_defaults_create_independent_properties_and_replay_original_omissions() {
    let (plain_before, plain_after, mut row) = fixtures::number_companions(false);
    // GUID generation/default expressions remain unqualified; a genuine absent
    // declaration resolves NULL while numeric defaults retain their exact bits.
    row[5] = Value::Null;
    let defaults = [
        ("ByteValue", "255"),
        ("ShortValue", "-32768"),
        ("SingleValue", "1e-45"),
        ("DoubleValue", "-0e0"),
        ("GuidValue", "Null"),
        ("DecimalValue", "-12.3450"),
    ];
    let before = with_default_expressions(plain_before, &defaults);
    let mut expected = with_default_expressions(plain_after, &defaults);
    // Independently authored non-NULL GUID bytes become an absent fixed span,
    // with only its corresponding mask bit cleared in the raw row.
    let data = &mut expected[8 * 4096..9 * 4096];
    let start = u16::from_le_bytes(data[14..16].try_into().unwrap()) as usize;
    data[start + 21..start + 37].fill(0);
    data[4095] = 95;
    let values = [
        Value::Byte(255),
        Value::Int16(i16::MIN),
        Value::Single(Single::from_bits(1)),
        Value::Double(Double::from_bits(0x8000_0000_0000_0000)),
        Value::Null,
        Value::Decimal(Decimal::new(-123450, 4).unwrap()),
    ];
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("ByteValue", ScalarType::Byte)
            .with_default_literal(&values[0])
            .unwrap(),
        CreateColumn::new("ShortValue", ScalarType::Int16)
            .with_default_literal(&values[1])
            .unwrap(),
        CreateColumn::new("SingleValue", ScalarType::Single)
            .with_default_literal(&values[2])
            .unwrap(),
        CreateColumn::new("DoubleValue", ScalarType::Double)
            .with_default_literal(&values[3])
            .unwrap(),
        CreateColumn::new("GuidValue", ScalarType::Guid)
            .with_default_literal(&values[4])
            .unwrap(),
        CreateColumn::decimal("DecimalValue", DecimalConstraints::new(18, 4).unwrap())
            .with_default_literal(&values[5])
            .unwrap(),
    ];
    let created = create_ace16_database("Items", &columns, &[], &mut budget(), &|| false).unwrap();
    assert_eq!(created.as_bytes(), before);
    let omitted = || {
        vec![OmittedRowInsert::new(
            (0..7).map(|_| InsertFieldValue::Omitted).collect(),
        )]
    };
    let plan = image(&before)
        .plan_recoverable_ordinary_omitted_row_inserts("Items", omitted(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), expected);
    let supplied = image(&before)
        .plan_recoverable_ordinary_omitted_row_inserts(
            "Items",
            request(&row),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(supplied.after_bytes(), expected);
    assert_eq!(
        &plan.intent().encode(&mut budget(), &|| false).unwrap()[..8],
        b"TCNA0002"
    );
    let complete_nulls = vec![
        Value::Int32(1),
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
    ];
    let explicit = image(&before)
        .plan_recoverable_ordinary_omitted_row_inserts(
            "Items",
            request(&complete_nulls),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        read_table(
            &mut image(explicit.after_bytes()).reader(),
            scope(),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        &[complete_nulls]
    );
    for text in ["1/0", "1e999", "SomeUserFunction()", "NaN"] {
        let invalid = with_default_expressions(
            declarations::number_companions().bytes,
            &[("SingleValue", text)],
        );
        assert!(
            image(&invalid)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    omitted(),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "{text}"
        );
    }
}
