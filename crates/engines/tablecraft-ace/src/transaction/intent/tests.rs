//! Literal source qualification precedes codec/planner API acceptance.
mod literals;

use super::{NativeInt32EditIntent, NativeIntentError};
use crate::{EditError, InspectError, Int32Update};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

const TEST_HEADER_BYTES: usize = 72;
const TEST_UPDATE_BYTES: usize = 16;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 16_000_000)
}

fn requests(case: &literals::Literal) -> Option<Vec<Int32Update>> {
    case.updates
        .iter()
        .map(|&(row, expected, value)| {
            usize::try_from(row)
                .ok()
                .map(|row| Int32Update::new(row, expected, value))
        })
        .collect()
}

#[test]
fn qualify_independent_intent_literals_before_codec_calls() {
    assert_eq!(literals::CASES.len(), 6);
    for case in literals::CASES {
        let wire = case.wire;
        assert_eq!(&wire[..8], b"TCNI0001");
        assert_eq!(
            u32::from_le_bytes(wire[8..12].try_into().unwrap()) as usize,
            wire.len()
        );
        assert_eq!(&wire[12..16], &[1, 0, 0, 0]);
        assert_eq!(
            u64::from_le_bytes(wire[16..24].try_into().unwrap()),
            case.before.len() as u64
        );
        let mut hash = Sha256::new();
        hash.update(b"TCNI0001:BEFORE\0");
        hash.update((case.before.len() as u64).to_le_bytes());
        hash.update(case.before);
        assert_eq!(&wire[24..56], hash.finalize().as_slice());
        assert_eq!(
            u32::from_le_bytes(wire[56..60].try_into().unwrap()) as usize,
            case.table.len()
        );
        assert_eq!(
            u32::from_le_bytes(wire[60..64].try_into().unwrap()) as usize,
            case.column.len()
        );
        assert_eq!(
            u32::from_le_bytes(wire[64..68].try_into().unwrap()) as usize,
            case.updates.len()
        );
        assert_eq!(&wire[68..72], &[0; 4]);
        let table_end = TEST_HEADER_BYTES + case.table.len();
        let column_end = table_end + case.column.len();
        assert_eq!(&wire[TEST_HEADER_BYTES..table_end], case.table.as_bytes());
        assert_eq!(&wire[table_end..column_end], case.column.as_bytes());
        assert_eq!(
            wire.len(),
            column_end + TEST_UPDATE_BYTES * case.updates.len()
        );
        for (entry, &(row, expected, value)) in wire[column_end..]
            .as_chunks::<16>()
            .0
            .iter()
            .zip(case.updates)
        {
            assert_eq!(u64::from_le_bytes(entry[..8].try_into().unwrap()), row);
            assert_eq!(
                i32::from_le_bytes(entry[8..12].try_into().unwrap()),
                expected
            );
            assert_eq!(i32::from_le_bytes(entry[12..16].try_into().unwrap()), value);
        }
    }
}

#[test]
fn codec_matches_literal_bytes_preserves_moved_vec_and_original_request_law() {
    for case in literals::CASES {
        let Some(updates) = requests(case) else {
            assert!(NativeInt32EditIntent::decode(case.wire, &mut budget(), &|| false).is_err());
            continue;
        };
        let pointer = updates.as_ptr();
        let intent = NativeInt32EditIntent::new(
            case.table,
            case.column,
            updates,
            case.before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            intent.updates().as_ptr(),
            pointer,
            "{}: request Vec was copied",
            case.id
        );
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), case.wire);
        let decoded = NativeInt32EditIntent::decode(case.wire, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.table(), case.table);
        assert_eq!(decoded.column(), case.column);
        assert_eq!(decoded.before_length(), case.before.len() as u64);
        assert_eq!(
            decoded
                .updates()
                .iter()
                .map(|value| (value.row() as u64, value.expected(), value.value()))
                .collect::<Vec<_>>(),
            case.updates
        );
        decoded
            .verify_before(case.before, &mut budget(), &|| false)
            .unwrap();
        let mut wrong = case.before.to_vec();
        wrong[0] ^= 1;
        assert!(matches!(
            decoded.verify_before(&wrong, &mut budget(), &|| false),
            Err(NativeIntentError::BeforeMismatch)
        ));
    }
    assert_eq!(NativeInt32EditIntent::MAX_ENCODED_BYTES, 278600);
}

#[test]
fn malformed_records_and_duplicate_requests_never_gain_authority() {
    for case in literals::CASES {
        for end in 0..case.wire.len() {
            assert!(
                NativeInt32EditIntent::decode(&case.wire[..end], &mut budget(), &|| false).is_err(),
                "{} prefix{end}",
                case.id
            );
        }
    }
    let base = literals::CASES[0].wire;
    for (at, patch) in [
        (0, b"XCNI".as_slice()),
        (7, b"2".as_slice()),
        (12, &[2, 0]),
        (14, &[1, 0]),
        (68, &[1, 0, 0, 0]),
        (8, &[73, 0, 0, 0]),
        (16, &[0; 8]),
        (72, &[255]),
        (72, &[0]),
    ] {
        let mut bad = base.to_vec();
        bad[at..at + patch.len()].copy_from_slice(patch);
        assert!(NativeInt32EditIntent::decode(&bad, &mut budget(), &|| false).is_err());
    }
    let mut trailing = base.to_vec();
    trailing.push(0);
    assert!(NativeInt32EditIntent::decode(&trailing, &mut budget(), &|| false).is_err());
    trailing[8..12].copy_from_slice(&75_u32.to_le_bytes());
    assert!(NativeInt32EditIntent::decode(&trailing, &mut budget(), &|| false).is_err());
    let mut duplicate = literals::CASES[1].wire.to_vec();
    duplicate[114..122].copy_from_slice(&2_u64.to_le_bytes());
    assert!(matches!(
        NativeInt32EditIntent::decode(&duplicate, &mut budget(), &|| false),
        Err(NativeIntentError::Request(EditError::Conflict))
    ));
    let mut bad_binding = base.to_vec();
    bad_binding[24] ^= 1;
    let data = NativeInt32EditIntent::decode(&bad_binding, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        data.verify_before(b"abc", &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
}

#[test]
fn allocation_claims_are_checked_before_allocation_and_exact_pools_hold() {
    let base = literals::CASES[0].wire;
    for (at, bytes) in [
        (56, 8193_u32.to_le_bytes()),
        (60, u32::MAX.to_le_bytes()),
        (64, 16385_u32.to_le_bytes()),
    ] {
        let mut bad = base.to_vec();
        bad[at..at + 4].copy_from_slice(&bytes);
        let mut zero_bytes = ResourceBudget::new(0, 0, 10000);
        assert!(NativeInt32EditIntent::decode(&bad, &mut zero_bytes, &|| false).is_err());
        assert_eq!(zero_bytes.remaining(Resource::Bytes), 0);
    }
    let case = &literals::CASES[1];
    let mut measured = budget();
    let original = measured.remaining(Resource::Bytes);
    let steps = measured.remaining(Resource::Steps);
    let decoded = NativeInt32EditIntent::decode(case.wire, &mut measured, &|| false).unwrap();
    let used_bytes = original - measured.remaining(Resource::Bytes);
    let used_steps = steps - measured.remaining(Resource::Steps);
    let mut exact = ResourceBudget::new(used_bytes, 0, used_steps);
    NativeInt32EditIntent::decode(case.wire, &mut exact, &|| false).unwrap();
    for (bytes, steps) in [(used_bytes - 1, used_steps), (used_bytes, used_steps - 1)] {
        assert!(
            NativeInt32EditIntent::decode(
                case.wire,
                &mut ResourceBudget::new(bytes, 0, steps),
                &|| false
            )
            .is_err()
        );
    }
    let mut output = ResourceBudget::new(case.wire.len() as u64, 0, 10000);
    assert_eq!(decoded.encode(&mut output, &|| false).unwrap(), case.wire);
    assert_eq!(output.remaining(Resource::Bytes), 0);
    assert!(
        decoded
            .encode(
                &mut ResourceBudget::new(case.wire.len() as u64 - 1, 0, 10000),
                &|| false
            )
            .is_err()
    );
}

#[test]
fn late_cancellation_never_returns_a_partial_intent_or_wire_record() {
    let case = &literals::CASES[1];
    let polls = Cell::new(0);
    let decoded = NativeInt32EditIntent::decode(case.wire, &mut budget(), &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let terminal = polls.get();
    polls.set(0);
    assert!(matches!(
        NativeInt32EditIntent::decode(case.wire, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == terminal
        }),
        Err(NativeIntentError::Inspect(InspectError::Cancelled))
    ));
    polls.set(0);
    decoded
        .encode(&mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let terminal = polls.get();
    polls.set(0);
    assert!(matches!(
        decoded.encode(&mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == terminal
        }),
        Err(NativeIntentError::Inspect(InspectError::Cancelled))
    ));
}
