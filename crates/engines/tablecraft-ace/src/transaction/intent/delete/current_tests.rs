//! Independent current DELETE framing; wire data grants no source authority.
#[cfg(feature = "ace16-create")]
mod native_recovery;

use super::{NativeDeleteIntent, NativeIntentError};
use crate::{EditError, InspectError, RowDelete};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

const BEFORE: &[u8] = b"original raw source";

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 0, 100_000_000)
}

fn literal(before: &[u8], table: &str, rows: &[(u64, &[u8])]) -> Vec<u8> {
    let length = 68
        + table.len()
        + rows
            .iter()
            .map(|(_, bytes)| 12 + bytes.len())
            .sum::<usize>();
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);

    let mut wire = b"TCND0002".to_vec();
    wire.extend((length as u32).to_le_bytes());
    wire.extend([2, 0, 0, 0]);
    wire.extend((before.len() as u64).to_le_bytes());
    wire.extend(digest.finalize());
    wire.extend((table.len() as u32).to_le_bytes());
    wire.extend((rows.len() as u32).to_le_bytes());
    wire.extend([0; 4]);
    wire.extend(table.as_bytes());
    for (ordinal, expected) in rows {
        wire.extend(ordinal.to_le_bytes());
        wire.extend((expected.len() as u32).to_le_bytes());
        wire.extend(*expected);
    }
    assert_eq!(wire.len(), length);
    wire
}

#[test]
fn current_delete_wire_keeps_written_order_binding_and_legacy_decoders_closed() {
    let wire = literal(BEFORE, "Items", &[(7, &[2, 0, 3]), (1, &[3, 0, 0, 7])]);
    assert_eq!(wire.len(), 104);
    let intent = NativeDeleteIntent::new_current(
        "Items",
        vec![
            RowDelete::new(7, vec![2, 0, 3]),
            RowDelete::new(1, vec![3, 0, 0, 7]),
        ],
        BEFORE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
    let decoded = NativeDeleteIntent::decode_current(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.table(), "Items");
    assert_eq!(decoded.deletes()[0].row(), 7);
    assert_eq!(decoded.deletes()[1].row(), 1);
    assert_eq!(decoded.deletes()[0].expected_bytes(), &[2, 0, 3]);
    assert_eq!(decoded.deletes()[1].expected_bytes(), &[3, 0, 0, 7]);
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    decoded
        .verify_before(BEFORE, &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        decoded.verify_before(b"modified raw source", &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
    assert!(matches!(
        NativeDeleteIntent::decode(&wire, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));

    let mut legacy = wire.to_vec();
    legacy[..8].copy_from_slice(b"TCND0001");
    legacy[12..14].copy_from_slice(&1_u16.to_le_bytes());
    assert!(NativeDeleteIntent::decode(&legacy, &mut budget(), &|| false).is_ok());
    assert!(matches!(
        NativeDeleteIntent::decode_current(&legacy, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));
    legacy[..8].copy_from_slice(b"TCND0003");
    assert!(matches!(
        NativeDeleteIntent::decode_current(&legacy, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));
}

#[test]
fn current_delete_malformed_and_duplicate_frames_never_publish_requests() {
    let wire = literal(BEFORE, "Items", &[(7, &[2, 0, 3]), (1, &[3, 0, 0, 7])]);
    for end in 0..wire.len() {
        assert!(
            NativeDeleteIntent::decode_current(&wire[..end], &mut budget(), &|| false).is_err()
        );
    }
    for offset in [8, 12, 14, 56, 60, 64, 68, 81] {
        let mut malformed = wire.to_vec();
        malformed[offset] ^= 0x80;
        if offset == 68 {
            malformed[offset] = 0xff;
        }
        assert!(NativeDeleteIntent::decode_current(&malformed, &mut budget(), &|| false).is_err());
    }
    let duplicate = literal(BEFORE, "Items", &[(7, &[2, 0, 3]), (7, &[3, 0, 0, 7])]);
    assert!(matches!(
        NativeDeleteIntent::decode_current(&duplicate, &mut budget(), &|| false),
        Err(NativeIntentError::Request(EditError::Conflict))
    ));
    assert!(matches!(
        NativeDeleteIntent::new_current(
            "Items",
            vec![
                RowDelete::new(7, vec![2, 0, 3]),
                RowDelete::new(7, vec![2, 0, 3])
            ],
            BEFORE,
            &mut budget(),
            &|| false,
        ),
        Err(NativeIntentError::Request(EditError::Conflict))
    ));
    for length in [0, 2, 4097] {
        let payload = vec![0; length];
        let malformed = literal(BEFORE, "Items", &[(0, &payload)]);
        assert!(NativeDeleteIntent::decode_current(&malformed, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn current_delete_sparse_ordinals_and_counts_have_no_inherited_sixteen_thousand_quota() {
    let requests = (0..16_385)
        .map(|position| RowDelete::new(16_384 + position * 3, vec![2, 0, 1]))
        .collect();
    let intent =
        NativeDeleteIntent::new_current("Items", requests, BEFORE, &mut budget(), &|| false)
            .unwrap();
    let wire = intent.encode(&mut budget(), &|| false).unwrap();
    let decoded = NativeDeleteIntent::decode_current(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.deletes().len(), 16_385);
    assert_eq!(decoded.deletes().first().unwrap().row(), 16_384);
    assert_eq!(decoded.deletes().last().unwrap().row(), 65_536);
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    assert!(
        NativeDeleteIntent::new(
            "Items",
            vec![RowDelete::new(16_384, vec![2, 0, 1])],
            BEFORE,
            &mut budget(),
            &|| false,
        )
        .is_err()
    );

    let high = literal(BEFORE, "Items", &[(usize::MAX as u64, &[2, 0, 1])]);
    let decoded = NativeDeleteIntent::decode_current(&high, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.deletes()[0].row(), usize::MAX);
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), high);
}

#[test]
fn current_delete_exact_resource_pools_and_cancellation_keep_empty_bindings_explicit() {
    let wire = literal(BEFORE, "Items", &[(7, &[2, 0, 3]), (1, &[3, 0, 0, 7])]);
    let mut measured = budget();
    let initial_bytes = measured.remaining(Resource::Bytes);
    let initial_steps = measured.remaining(Resource::Steps);
    let polls = Cell::new(0);
    NativeDeleteIntent::decode_current(&wire, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let bytes = initial_bytes - measured.remaining(Resource::Bytes);
    let steps = initial_steps - measured.remaining(Resource::Steps);
    NativeDeleteIntent::decode_current(&wire, &mut ResourceBudget::new(bytes, 0, steps), &|| false)
        .unwrap();
    for (byte_limit, step_limit) in [(bytes - 1, steps), (bytes, steps - 1)] {
        assert!(
            NativeDeleteIntent::decode_current(
                &wire,
                &mut ResourceBudget::new(byte_limit, 0, step_limit),
                &|| false,
            )
            .is_err()
        );
    }
    for stop in [1, polls.get() / 2, polls.get()] {
        let count = Cell::new(0);
        assert!(matches!(
            NativeDeleteIntent::decode_current(&wire, &mut budget(), &|| {
                count.set(count.get() + 1);
                count.get() == stop
            }),
            Err(NativeIntentError::Inspect(InspectError::Cancelled))
        ));
    }
    let empty =
        NativeDeleteIntent::new_current("Items", Vec::new(), BEFORE, &mut budget(), &|| false)
            .unwrap();
    let raw = empty.encode(&mut budget(), &|| false).unwrap();
    assert_eq!(raw, literal(BEFORE, "Items", &[]));
    let decoded = NativeDeleteIntent::decode_current(&raw, &mut budget(), &|| false).unwrap();
    assert!(decoded.deletes().is_empty());
    decoded
        .verify_before(BEFORE, &mut budget(), &|| false)
        .unwrap();
}
