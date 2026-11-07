//! Independent TCNA2 data-only framing, not native default permission.
use super::{NativeIntentError, NativeOmittedInsertIntent};
use crate::{InsertFieldValue, InspectError, NativeInsertIntent, OmittedRowInsert};
use std::{array, cell::Cell};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, Guid, LegacyDateTime, Single, Value},
};

use tablecraft_testkit::fixtures::access::tcna2;

const POOLS: [Resource; 2] = [Resource::Bytes, Resource::Steps];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn requests() -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Supplied(Value::Int32(24)),
        InsertFieldValue::Omitted,
        InsertFieldValue::Supplied(Value::Null),
    ])]
}

fn literal() -> Vec<u8> {
    let mut raw = Vec::from(*b"TCNA0002");
    raw.extend_from_slice(&112_u32.to_le_bytes());
    raw.extend_from_slice(&[2, 0, 0, 0]);
    raw.extend_from_slice(&8_u64.to_le_bytes());
    let binding = "da0a5f300db10f6a7396b20b324a8f569ea50cdbc8416d136c45a933dc2e35f3";
    for pair in binding.as_bytes().as_chunks::<2>().0 {
        raw.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
    }
    for value in [8_u32, 1, 0] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.extend_from_slice(b"Defaults");
    raw.extend_from_slice(&36_u32.to_le_bytes());
    raw.extend_from_slice(&3_u32.to_le_bytes());
    raw.extend_from_slice(&[2, 0, 0, 0, 4, 0, 0, 0, 24, 0, 0, 0]);
    raw.extend_from_slice(&[8, 0, 0, 0, 0, 0, 0, 0]);
    raw.extend_from_slice(&[0; 8]);
    assert_eq!(raw.len(), 112);
    raw
}

#[test]
fn exact_literal_wire_retains_presence_and_moves_original_rows_without_expansion() {
    let rows = requests();
    let pointer = rows[0].values().as_ptr();
    let intent =
        NativeOmittedInsertIntent::new("Defaults", rows, b"original", &mut budget(), &|| false)
            .unwrap();
    assert_eq!(intent.rows()[0].values().as_ptr(), pointer);
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), literal());
    let decoded = NativeOmittedInsertIntent::decode(&literal(), &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.table(), "Defaults");
    assert!(matches!(
        decoded.rows()[0].values()[0],
        InsertFieldValue::Supplied(Value::Int32(24))
    ));
    assert!(matches!(
        decoded.rows()[0].values()[1],
        InsertFieldValue::Omitted
    ));
    assert!(matches!(
        decoded.rows()[0].values()[2],
        InsertFieldValue::Supplied(Value::Null)
    ));
    decoded
        .verify_before(b"original", &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        decoded.verify_before(b"differnt", &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
    assert!(matches!(
        NativeInsertIntent::decode(&literal(), &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));
}

#[test]
fn exact_fixed_variable_and_null_carriers_remain_supplied_not_omitted() {
    let values = vec![
        Value::Null,
        Value::Boolean(false),
        Value::Int32(i32::MIN),
        Value::Int64(i64::MAX),
        Value::Currency(Currency::from_scaled(-10001)),
        Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
        Value::Text(String::new()),
        Value::Binary(Vec::new()),
        Value::Text("雪🙂".to_owned()),
        Value::Binary(vec![0, 255]),
    ];
    let mut presence = vec![InsertFieldValue::Omitted];
    presence.extend(values.iter().cloned().map(InsertFieldValue::Supplied));
    let intent = NativeOmittedInsertIntent::new(
        "Defaults",
        vec![OmittedRowInsert::new(presence)],
        b"original",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let raw = intent.encode(&mut budget(), &|| false).unwrap();
    let decoded = NativeOmittedInsertIntent::decode(&raw, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        decoded.rows()[0].values()[0],
        InsertFieldValue::Omitted
    ));
    for (field, expected) in decoded.rows()[0].values()[1..].iter().zip(&values) {
        let InsertFieldValue::Supplied(actual) = field else {
            panic!("Supplied data was replaced by an omitted marker");
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn malformed_omitted_payload_padding_width_tags_and_truncation_refuse_before_allocation() {
    let good = literal();
    let mut malformed = (0..good.len())
        .map(|end| good[..end].to_vec())
        .collect::<Vec<_>>();
    for (offset, value) in [
        (12, 1),
        (14, 1),
        (64, 1),
        (76, 37),
        (80, 0),
        (96, 9),
        (97, 1),
        (100, 1),
        (104, 255),
    ] {
        let mut raw = good.clone();
        raw[offset] = value;
        malformed.push(raw);
    }
    for (offset, value) in [(60, 16385_u32), (80, 257)] {
        let mut raw = good.clone();
        raw[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        malformed.push(raw);
    }
    let mut extra = good.clone();
    extra.push(0);
    extra[8..12].copy_from_slice(&113_u32.to_le_bytes());
    malformed.push(extra);
    for raw in malformed {
        let error = NativeOmittedInsertIntent::decode(
            &raw,
            &mut ResourceBudget::new(0, 0, 1_000_000),
            &|| false,
        )
        .err()
        .unwrap();
        assert!(!matches!(
            error,
            NativeIntentError::Inspect(InspectError::Limit(_))
        ));
    }
    for magic in [b"TCNI0001", b"TCND0001", b"TCNA0001", b"TCNA0003"] {
        let mut raw = good.clone();
        raw[..8].copy_from_slice(magic);
        assert!(NativeOmittedInsertIntent::decode(&raw, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn empty_request_batch_is_retained_but_empty_or_inconsistent_rows_refuse() {
    let empty =
        NativeOmittedInsertIntent::new("Defaults", vec![], b"original", &mut budget(), &|| false)
            .unwrap();
    let decoded = NativeOmittedInsertIntent::decode(
        &empty.encode(&mut budget(), &|| false).unwrap(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(decoded.rows().is_empty());
    assert!(
        NativeOmittedInsertIntent::new(
            "Defaults",
            vec![OmittedRowInsert::new(vec![])],
            b"original",
            &mut budget(),
            &|| false,
        )
        .is_err()
    );
    assert!(
        NativeOmittedInsertIntent::new(
            "Defaults",
            vec![
                OmittedRowInsert::new(vec![InsertFieldValue::Omitted]),
                OmittedRowInsert::new(vec![InsertFieldValue::Omitted, InsertFieldValue::Omitted]),
            ],
            b"original",
            &mut budget(),
            &|| false,
        )
        .is_err()
    );
}

#[test]
fn encode_and_decode_pay_exact_metadata_pools_and_cancel_at_the_actual_final_poll() {
    let intent =
        NativeOmittedInsertIntent::new("Defaults", requests(), b"original", &mut budget(), &|| {
            false
        })
        .unwrap();
    for encode in [true, false] {
        let operation = |work: &mut ResourceBudget, cancelled: &dyn Fn() -> bool| {
            if encode {
                intent.encode(work, cancelled).map(|_| ())
            } else {
                NativeOmittedInsertIntent::decode(&literal(), work, cancelled).map(|_| ())
            }
        };
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        let polls = Cell::new(0_usize);
        operation(&mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        let costs: [u64; 2] =
            array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        assert!(costs.iter().all(|cost| *cost > 0));
        assert_eq!(measured.remaining(Resource::Rows), 0);
        let mut exact = ResourceBudget::new(costs[0], 0, costs[1]);
        operation(&mut exact, &|| false).unwrap();
        assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0]);
        for index in 0..POOLS.len() {
            let mut allowance = costs;
            allowance[index] -= 1;
            assert!(matches!(
                operation(&mut ResourceBudget::new(allowance[0], 0, allowance[1]), &|| false),
                Err(NativeIntentError::Inspect(InspectError::Limit(error))) if error.resource == POOLS[index]
            ));
        }
        for stop in [1, polls.get().div_ceil(2), polls.get()] {
            let seen = Cell::new(0_usize);
            assert!(matches!(
                operation(&mut budget(), &|| {
                    seen.set(seen.get() + 1);
                    seen.get() >= stop
                }),
                Err(NativeIntentError::Inspect(InspectError::Cancelled))
            ));
            assert_eq!(seen.get(), stop);
        }
    }
}

#[test]
fn wider_original_presence_uses_distinct_closed_protocol_and_independent_tags() {
    let values = [
        Value::Byte(255),
        Value::Int16(i16::MIN),
        Value::Single(Single::from_bits(1)),
        Value::Double(Double::from_bits(0x8000_0000_0000_0000)),
        Value::Guid(Guid::from_bytes([0x11; 16])),
        Value::Decimal(Decimal::new(-123450, 4).unwrap()),
    ];
    let mut fields = vec![InsertFieldValue::Omitted];
    fields.extend(values.iter().cloned().map(InsertFieldValue::Supplied));
    let intent = NativeOmittedInsertIntent::new(
        "Defaults",
        vec![OmittedRowInsert::new(fields)],
        b"original",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut original = vec![None];
    original.extend(values.iter().cloned().map(Some));
    let binding = literal()[24..56].try_into().unwrap();
    let expected = tcna2::wider_presence_wire("Defaults", 8, binding, vec![original]);
    let raw = intent.encode(&mut budget(), &|| false).unwrap();
    assert_eq!(raw, expected);
    let decoded = NativeOmittedInsertIntent::decode(&raw, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), raw);
    for (original, decoded) in values.iter().zip(&decoded.rows()[0].values()[1..]) {
        assert!(matches!(decoded, InsertFieldValue::Supplied(value) if value == original));
    }
    for (offset, byte) in [(7, b'2'), (12, 2), (84, 8), (92, 14), (93, 1)] {
        let mut invalid = raw.clone();
        invalid[offset] = byte;
        assert!(
            NativeOmittedInsertIntent::decode(&invalid, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
}
