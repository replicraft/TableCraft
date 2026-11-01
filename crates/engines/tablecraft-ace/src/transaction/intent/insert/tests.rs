//! Literal data-only TCNA framing; these tests never manufacture native authority.
use super::{NativeInsertIntent, NativeIntentError};
use crate::{InspectError, RowInsert};
use std::{array, cell::Cell};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, LegacyDateTime, Value},
};

const POOLS: [Resource; 2] = [Resource::Bytes, Resource::Steps];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn rows() -> Vec<RowInsert> {
    vec![RowInsert::new(vec![
        Value::Int32(24),
        Value::Text("new".into()),
        Value::Null,
    ])]
}

fn literal() -> Vec<u8> {
    let mut raw = Vec::from(*b"TCNA0001");
    raw.extend_from_slice(&115_u32.to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&8_u64.to_le_bytes());
    let binding = "da0a5f300db10f6a7396b20b324a8f569ea50cdbc8416d136c45a933dc2e35f3";
    for pair in binding.as_bytes().as_chunks::<2>().0 {
        raw.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
    }
    for word in [8_u32, 1, 0] {
        raw.extend_from_slice(&word.to_le_bytes());
    }
    raw.extend_from_slice(b"Sentinel");
    raw.extend_from_slice(&39_u32.to_le_bytes());
    raw.extend_from_slice(&3_u32.to_le_bytes());
    raw.extend_from_slice(&[2, 0, 0, 0, 4, 0, 0, 0, 24, 0, 0, 0]);
    raw.extend_from_slice(&[6, 0, 0, 0, 3, 0, 0, 0]);
    raw.extend_from_slice(b"new");
    raw.extend_from_slice(&[0; 8]);
    assert_eq!(raw.len(), 115);
    raw
}

#[test]
fn literal_insert_wire_keeps_moved_original_rows_and_distinct_action_family() {
    let supplied = rows();
    let values = supplied[0].values().as_ptr();
    let intent =
        NativeInsertIntent::new("Sentinel", supplied, b"original", &mut budget(), &|| false)
            .unwrap();
    assert_eq!(intent.rows()[0].values().as_ptr(), values);
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), literal());
    let decoded = NativeInsertIntent::decode(&literal(), &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.table(), "Sentinel");
    assert_eq!(decoded.rows()[0].values(), rows()[0].values());
    decoded
        .verify_before(b"original", &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        decoded.verify_before(b"differnt", &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
    for prefix in [b"TCNI0001", b"TCND0001", b"TCNA0002"] {
        let mut wrong = literal();
        wrong[..8].copy_from_slice(prefix);
        assert!(NativeInsertIntent::decode(&wrong, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn carrier_union_preserves_nullable_empty_binary_and_exact_fixed_representations() {
    let values = vec![
        Value::Boolean(false),
        Value::Int32(-7),
        Value::Int64(i64::MIN),
        Value::Currency(Currency::from_scaled(-10_001)),
        Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
        Value::Null,
        Value::Text(String::new()),
        Value::Binary(Vec::new()),
        Value::Text("雪🙂".into()),
        Value::Binary(vec![0, 255]),
    ];
    let original = NativeInsertIntent::new(
        "Rows",
        vec![RowInsert::new(values.clone())],
        b"original",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let raw = original.encode(&mut budget(), &|| false).unwrap();
    let decoded = NativeInsertIntent::decode(&raw, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.rows()[0].values(), values);
    let empty =
        NativeInsertIntent::new("Rows", vec![], b"original", &mut budget(), &|| false).unwrap();
    assert!(
        NativeInsertIntent::decode(
            &empty.encode(&mut budget(), &|| false).unwrap(),
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows()
        .is_empty()
    );
    assert!(
        NativeInsertIntent::new(
            "Rows",
            vec![RowInsert::new(vec![])],
            b"original",
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert!(
        NativeInsertIntent::new(
            "Rows",
            vec![
                RowInsert::new(vec![Value::Null]),
                RowInsert::new(vec![Value::Null, Value::Null])
            ],
            b"original",
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn malformed_rows_and_carriers_refuse_before_decoded_allocation() {
    let good = literal();
    let mut controls = Vec::new();
    for end in 0..good.len() {
        controls.push(good[..end].to_vec());
    }
    for (offset, value) in [
        (12, 2),
        (14, 1),
        (64, 1),
        (76, 40),
        (80, 0),
        (84, 255),
        (85, 1),
        (104, 255),
    ] {
        let mut bad = good.clone();
        bad[offset] = value;
        controls.push(bad);
    }
    let mut excess = good.clone();
    excess[60..64].copy_from_slice(&16_385_u32.to_le_bytes());
    controls.push(excess);
    let mut excess = good.clone();
    excess[80..84].copy_from_slice(&257_u32.to_le_bytes());
    controls.push(excess);
    let mut trailing = good;
    trailing.push(0);
    trailing[8..12].copy_from_slice(&116_u32.to_le_bytes());
    controls.push(trailing);
    for bad in controls {
        let error =
            NativeInsertIntent::decode(&bad, &mut ResourceBudget::new(0, 0, 1_000_000), &|| false)
                .err()
                .unwrap();
        assert!(
            !matches!(error, NativeIntentError::Inspect(InspectError::Limit(_))),
            "malformed framing reached allocation"
        );
    }
}

#[test]
fn encoding_and_decoding_pay_exact_pools_and_observe_all_cancellation_boundaries() {
    let intent =
        NativeInsertIntent::new("Sentinel", rows(), b"original", &mut budget(), &|| false).unwrap();
    for decode in [false, true] {
        let operation = |work: &mut ResourceBudget, cancel: &dyn Fn() -> bool| {
            if decode {
                NativeInsertIntent::decode(&literal(), work, cancel).map(|_| ())
            } else {
                intent.encode(work, cancel).map(|_| ())
            }
        };
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        operation(&mut measured, &|| false).unwrap();
        let costs: [u64; 2] =
            array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        let mut exact = ResourceBudget::new(costs[0], 0, costs[1]);
        operation(&mut exact, &|| false).unwrap();
        assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0]);
        for index in 0..2 {
            let mut below = costs;
            below[index] -= 1;
            let error = operation(&mut ResourceBudget::new(below[0], 0, below[1]), &|| false)
                .err()
                .unwrap();
            assert!(
                matches!(error, NativeIntentError::Inspect(InspectError::Limit(error)) if error.resource == POOLS[index])
            );
        }
        let polls = Cell::new(0_usize);
        operation(&mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        for stop in [1, polls.get().div_ceil(2), polls.get()] {
            let seen = Cell::new(0_usize);
            let error = operation(&mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            })
            .err()
            .unwrap();
            assert!(matches!(
                error,
                NativeIntentError::Inspect(InspectError::Cancelled)
            ));
        }
    }
}
