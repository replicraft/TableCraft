//! Independent literal request framing; no native producer or storage authority.
use super::{NativeCompleteInsertIntent, NativeIntentError};
use crate::{InspectError, RowInsert};
use std::{array, cell::Cell};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Decimal, Double, Guid, Single, Value},
};

const POOLS: [Resource; 2] = [Resource::Bytes, Resource::Steps];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn rows() -> Vec<RowInsert> {
    vec![RowInsert::new(vec![
        Value::Byte(255),
        Value::Int16(-32768),
        Value::Single(Single::from_bits(0x7f81_2345)),
        Value::Double(Double::from_bits(0xfff8_1234_5678_9abc)),
        Value::Guid(Guid::from_bytes([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ])),
        Value::Decimal(Decimal::new(-12345, 4).unwrap()),
    ])]
}

fn literal() -> Vec<u8> {
    let mut raw = Vec::from(*b"TCNA0004");
    raw.extend_from_slice(&180_u32.to_le_bytes());
    raw.extend_from_slice(&[4, 0, 0, 0]);
    raw.extend_from_slice(&8_u64.to_le_bytes());
    let binding = "da0a5f300db10f6a7396b20b324a8f569ea50cdbc8416d136c45a933dc2e35f3";
    for pair in binding.as_bytes().as_chunks::<2>().0 {
        raw.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
    }
    for word in [8_u32, 1, 0] {
        raw.extend_from_slice(&word.to_le_bytes());
    }
    raw.extend_from_slice(b"Sentinel");
    raw.extend_from_slice(&104_u32.to_le_bytes());
    raw.extend_from_slice(&6_u32.to_le_bytes());
    let cells: [(u8, &[u8]); 6] = [
        (8, &[255]),
        (9, &[0, 128]),
        (10, &[0x45, 0x23, 0x81, 0x7f]),
        (11, &[0xbc, 0x9a, 0x78, 0x56, 0x34, 0x12, 0xf8, 0xff]),
        (
            12,
            &[
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ],
        ),
        (
            13,
            &[
                0xc7, 0xcf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 4,
            ],
        ),
    ];
    for (tag, bytes) in cells {
        raw.extend_from_slice(&[tag, 0, 0, 0]);
        raw.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        raw.extend_from_slice(bytes);
    }
    assert_eq!(raw.len(), 180);
    raw
}

#[test]
fn literal_tcna4_retains_wide_representations_and_original_binding() {
    let supplied = rows();
    let pointer = supplied[0].values().as_ptr();
    let intent =
        NativeCompleteInsertIntent::new("Sentinel", supplied, b"original", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(intent.rows()[0].values().as_ptr(), pointer);
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), literal());
    let decoded = NativeCompleteInsertIntent::decode(&literal(), &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.rows()[0].values(), rows()[0].values());
    decoded
        .verify_before(b"original", &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        decoded.verify_before(b"differnt", &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
    for prefix in [b"TCNA0001", b"TCNA0002", b"TCNA0003", b"TCNA0005"] {
        let mut wrong = literal();
        wrong[..8].copy_from_slice(prefix);
        assert!(NativeCompleteInsertIntent::decode(&wrong, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn wide_lengths_padding_and_invalid_decimal_refuse_before_allocation() {
    let good = literal();
    let mut controls = Vec::new();
    for end in 0..good.len() {
        controls.push(good[..end].to_vec());
    }
    for (at, byte) in [
        (14, 1),
        (64, 1),
        (84, 14),
        (85, 1),
        (88, 2),
        (97, 1),
        (107, 3),
        (119, 7),
        (135, 15),
        (159, 16),
        (179, 29),
    ] {
        let mut bad = good.clone();
        bad[at] = byte;
        controls.push(bad);
    }
    let mut overflow = good.clone();
    overflow[163..179].copy_from_slice(&i128::MIN.to_le_bytes());
    controls.push(overflow);
    for bad in controls {
        let error = NativeCompleteInsertIntent::decode(
            &bad,
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
}

#[test]
fn encoding_and_decoding_pay_exact_pools_and_observe_all_cancellation_boundaries() {
    let intent =
        NativeCompleteInsertIntent::new("Sentinel", rows(), b"original", &mut budget(), &|| false)
            .unwrap();
    for decode in [false, true] {
        let operation = |work: &mut ResourceBudget, cancel: &dyn Fn() -> bool| {
            if decode {
                NativeCompleteInsertIntent::decode(&literal(), work, cancel).map(|_| ())
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

#[test]
fn legacy_complete_protocols_keep_their_closed_carrier_grammar() {
    use super::super::complete_insert::{CompleteRows, Protocol};

    for value in rows().remove(0).values() {
        assert!(
            CompleteRows::new(
                "Sentinel",
                vec![RowInsert::new(vec![value.clone()])],
                b"original",
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
    }
    for (magic, profile, protocol) in [
        (b"TCNA0001", 1_u16, Protocol::Existing),
        (b"TCNA0003", 3_u16, Protocol::Eof),
    ] {
        let mut wrong = literal();
        wrong[..8].copy_from_slice(magic);
        wrong[12..14].copy_from_slice(&profile.to_le_bytes());
        assert!(CompleteRows::decode(&wrong, protocol, &mut budget(), &|| false).is_err());
    }
}
