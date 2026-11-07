use super::{FirstTableIntentError, NativeFirstTableIntent};
use crate::{CreateColumn, FirstTableDefinition, FirstTablePlan, NativeImage};
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation, canonical_initial_table_creation as fixtures,
};

const POOLS: [Resource; 2] = [Resource::Bytes, Resource::Steps];

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn plan() -> FirstTablePlan {
    let columns = [CreateColumn::new("Id", ScalarType::Int32)
        .with_primary_key("PrimaryId")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap()];
    let definition =
        FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap();
    NativeImage::capture(
        &mut Cursor::new(canonical_blank_creation::source()),
        CatalogScope::new(482),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_first_table(definition, &mut budget(), &|| false)
    .unwrap()
}

fn literal() -> Vec<u8> {
    let before = canonical_blank_creation::source();
    assert_eq!(before.len(), 28_672);
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update(28_672_u64.to_le_bytes());
    hash.update(&before);

    let mut raw = Vec::from(*b"TCNT0001");
    raw.extend(88_u32.to_le_bytes());
    raw.extend([1, 0, 0, 0]);
    raw.extend(28_672_u64.to_le_bytes());
    raw.extend(hash.finalize());
    for value in [5_u16, 9, 1, 0] {
        raw.extend(value.to_le_bytes());
    }
    raw.extend(b"ItemsPrimaryId");
    raw.extend([2, 0, 0, 0, 0, 0, 0, 0]);
    raw.extend(b"Id");
    assert_eq!(raw.len(), 88);
    raw
}

#[test]
fn first_table_intent_matches_independent_original_only_wire() {
    assert_eq!(
        plan().encode_intent(&mut budget(), &|| false).unwrap(),
        literal()
    );
    let decoded = NativeFirstTableIntent::decode(&literal(), &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.table(), "Items");
    assert_eq!(decoded.column_count(), 1);
    decoded
        .verify_before(&canonical_blank_creation::source(), &mut budget(), &|| {
            false
        })
        .unwrap();
    let mut changed = canonical_blank_creation::source();
    *changed.last_mut().unwrap() ^= 1;
    assert!(matches!(
        decoded.verify_before(&changed, &mut budget(), &|| false),
        Err(FirstTableIntentError::BeforeMismatch)
    ));
}

#[test]
fn closed_schema_framing_rejects_unknown_versions_flags_tags_parameters_and_tails() {
    let raw = literal();
    for length in 0..raw.len() {
        assert!(NativeFirstTableIntent::decode(&raw[..length], &mut budget(), &|| false).is_err());
    }
    for (at, value) in [
        (0, b'X'),
        (7, b'2'),
        (8, 87),
        (12, 2),
        (14, 1),
        (16, 1),
        (24, 0),
        (56, 0),
        (58, 0),
        (60, 0),
        (60, 17),
        (62, 1),
        (64, b'.'),
        (78, 255),
        (80, 4),
        (80, 255),
        (81, 1),
        (82, 1),
        (84, 1),
        (85, 1),
        (86, 255),
    ] {
        let mut invalid = raw.clone();
        invalid[at] = value;
        assert!(
            NativeFirstTableIntent::decode(&invalid, &mut budget(), &|| false).is_err(),
            "offset {at}"
        );
    }
    let mut tail = raw.clone();
    tail.push(0);
    tail[8..12].copy_from_slice(&89_u32.to_le_bytes());
    assert!(NativeFirstTableIntent::decode(&tail, &mut budget(), &|| false).is_err());
    let over = vec![0; NativeFirstTableIntent::MAX_ENCODED_BYTES + 1];
    assert!(NativeFirstTableIntent::decode(&over, &mut budget(), &|| false).is_err());
}

#[test]
fn schema_intent_encode_and_decode_pay_exact_pools_and_poll_cancellation() {
    assert_intent_resources(plan(), literal());
}

fn assert_intent_resources(plan: FirstTablePlan, literal: Vec<u8>) {
    for decoding in [false, true] {
        let invoke = |work: &mut ResourceBudget, cancelled: &dyn Fn() -> bool| {
            if decoding {
                NativeFirstTableIntent::decode(&literal, work, cancelled).map(|_| ())
            } else {
                plan.encode_intent(work, cancelled).map(|_| ())
            }
        };
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        let polls = Cell::new(0_usize);
        invoke(&mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        let used = [
            initial[0] - measured.remaining(POOLS[0]),
            initial[1] - measured.remaining(POOLS[1]),
        ];
        assert!(used.iter().all(|cost| *cost > 0));
        invoke(&mut ResourceBudget::new(used[0], 0, used[1]), &|| false).unwrap();
        for short in [
            ResourceBudget::new(used[0] - 1, 0, used[1]),
            ResourceBudget::new(used[0], 0, used[1] - 1),
        ] {
            let mut short = short;
            assert!(invoke(&mut short, &|| false).is_err());
        }
        let terminal = polls.get();
        for stop in [1, terminal.div_ceil(2), terminal] {
            let seen = Cell::new(0_usize);
            assert!(
                invoke(&mut budget(), &|| {
                    seen.set(seen.get() + 1);
                    seen.get() >= stop
                })
                .is_err()
            );
            assert_eq!(seen.get(), stop);
        }
    }
}

#[test]
fn schema_intent_retains_every_constructible_companion_tag_and_parameter() {
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Flag", ScalarType::Boolean),
        CreateColumn::new("ByteValue", ScalarType::Byte),
        CreateColumn::new("ShortValue", ScalarType::Int16),
        CreateColumn::new("LongValue", ScalarType::Int32),
        CreateColumn::new("LargeValue", ScalarType::Int64),
        CreateColumn::new("Money", ScalarType::Currency),
        CreateColumn::new("SingleValue", ScalarType::Single),
        CreateColumn::new("DoubleValue", ScalarType::Double),
        CreateColumn::new("DateValue", ScalarType::DateTime),
        CreateColumn::new("GuidValue", ScalarType::Guid),
        CreateColumn::short_text("TextValue", 20).unwrap(),
        CreateColumn::short_binary("BinaryValue", 16).unwrap(),
        CreateColumn::decimal("DecimalValue", DecimalConstraints::new(18, 4).unwrap()),
    ];
    let before = canonical_blank_creation::source();
    let expected = fixtures::source(fixtures::Case::Mixed);
    let definition =
        FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap();
    let source = || {
        NativeImage::capture(
            &mut Cursor::new(&before),
            CatalogScope::new(483),
            &mut budget(),
            &|| false,
        )
        .unwrap()
    };
    let original = source()
        .plan_first_table(definition, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(original.after_bytes(), expected.bytes);
    let raw = original.encode_intent(&mut budget(), &|| false).unwrap();
    let mut at = 64 + 5 + 9;
    for tag in 0..14 {
        let length = usize::from(u16::from_le_bytes(raw[at..at + 2].try_into().unwrap()));
        assert_eq!(raw[at + 2], tag);
        assert_eq!(raw[at + 3], 0);
        let parameters = &raw[at + 4..at + 8];
        assert_eq!(
            parameters,
            match tag {
                11 => &[20, 0, 0, 0],
                12 => &[16, 0, 0, 0],
                13 => &[0, 0, 18, 4],
                _ => &[0, 0, 0, 0],
            }
        );
        at += 8 + length;
    }
    assert_eq!(at, raw.len());
    let decoded = NativeFirstTableIntent::decode(&raw, &mut budget(), &|| false).unwrap();
    let rebuilt = source()
        .plan_first_table(decoded.definition, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(rebuilt.after_bytes(), expected.bytes);
}

#[test]
fn default_schema_v2_matches_independent_typed_wire_and_retains_exact_original_values() {
    use tablecraft_core::values::{Currency, LegacyDateTime, Value};

    let values = [
        Value::Null,
        Value::Boolean(false),
        Value::Int32(-7),
        Value::Int64(i64::MAX),
        Value::Currency(Currency::from_scaled(12345)),
        Value::DateTime(LegacyDateTime::from_bits(0_f64.to_bits()).unwrap()),
        Value::Text("a\"b".into()),
    ];
    let primary = CreateColumn::new("Id", ScalarType::Int32)
        .with_primary_key("PrimaryId")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    let declarations = [
        CreateColumn::new("Nullable", ScalarType::Int32),
        CreateColumn::new("Flag", ScalarType::Boolean),
        CreateColumn::new("Long", ScalarType::Int32),
        CreateColumn::new("Large", ScalarType::Int64),
        CreateColumn::new("Money", ScalarType::Currency),
        CreateColumn::new("When", ScalarType::DateTime),
        CreateColumn::short_text("Text", 20).unwrap(),
    ];
    let mut columns = vec![primary];
    columns.extend(
        declarations
            .iter()
            .zip(&values)
            .map(|(column, value)| column.with_default_literal(value).unwrap()),
    );
    let definition =
        FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap();
    let source = || {
        NativeImage::capture(
            &mut Cursor::new(canonical_blank_creation::source()),
            CatalogScope::new(489),
            &mut budget(),
            &|| false,
        )
        .unwrap()
    };
    let plan = source()
        .plan_first_table(definition, &mut budget(), &|| false)
        .unwrap();
    let raw = plan.encode_intent(&mut budget(), &|| false).unwrap();
    let mut expected = literal();
    expected[..8].copy_from_slice(b"TCNT0002");
    expected[12..14].copy_from_slice(&2_u16.to_le_bytes());
    expected[60..62].copy_from_slice(&8_u16.to_le_bytes());
    let payloads: [&[u8]; 7] = [
        &[],
        &[0],
        &(-7_i32).to_le_bytes(),
        &i64::MAX.to_le_bytes(),
        &12345_i64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        b"a\"b",
    ];
    for (position, ((name, scalar_tag), payload)) in [
        ("Nullable", 4),
        ("Flag", 1),
        ("Long", 4),
        ("Large", 5),
        ("Money", 6),
        ("When", 9),
        ("Text", 11),
    ]
    .into_iter()
    .zip(payloads)
    .enumerate()
    {
        expected.extend((name.len() as u16).to_le_bytes());
        expected.extend([scalar_tag, 1]);
        expected.extend(if scalar_tag == 11 { 20_u16 } else { 0 }.to_le_bytes());
        expected.extend([0, 0]);
        expected.extend(name.as_bytes());
        expected.extend([position as u8, 0, 0, 0]);
        expected.extend((payload.len() as u32).to_le_bytes());
        expected.extend(payload);
    }
    let total = expected.len() as u32;
    expected[8..12].copy_from_slice(&total.to_le_bytes());
    assert_eq!(raw, expected);
    let decoded = NativeFirstTableIntent::decode(&expected, &mut budget(), &|| false).unwrap();
    let retained = decoded
        .definition
        .borrowed_columns(&mut budget(), &|| false)
        .unwrap();
    assert!(retained[0].default_literal().is_none());
    for (column, expected) in retained[1..].iter().zip(&values) {
        assert_eq!(column.default_literal(), Some(expected));
    }
    drop(retained);
    let rebuilt = source()
        .plan_first_table(decoded.definition, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(rebuilt.after_bytes(), plan.after_bytes());
    for at in [81, 106] {
        let mut invalid = raw.clone();
        invalid[at] = 2;
        assert!(NativeFirstTableIntent::decode(&invalid, &mut budget(), &|| false).is_err());
    }
    let mut no_defaults = literal();
    no_defaults[..8].copy_from_slice(b"TCNT0002");
    no_defaults[12..14].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        NativeFirstTableIntent::decode(&no_defaults, &mut budget(), &|| false),
        Err(FirstTableIntentError::InvalidRecord)
    ));
    for length in 0..raw.len() {
        assert!(NativeFirstTableIntent::decode(&raw[..length], &mut budget(), &|| false).is_err());
    }
    assert_intent_resources(plan, raw);
}

mod policies;

mod numbers;
