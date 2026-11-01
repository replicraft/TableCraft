//! Independent framing and source-binding controls; no native save permission.
use super::{NativeDeleteIntent, NativeIntentError};
use crate::{EditError, InspectError, RowDelete};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

const BEFORE: &[u8] = b"original raw source";

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn literal() -> Vec<u8> {
    // Independent protocol construction: 68-byte header, name and two ordered
    // raw row frames. No production offsets, tags or encoder are consumed.
    let mut raw = b"TCND0001".to_vec();
    raw.extend_from_slice(&104_u32.to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&19_u64.to_le_bytes());
    let mut binding = Sha256::new();
    binding.update(b"TCNI0001:BEFORE\0");
    binding.update(19_u64.to_le_bytes());
    binding.update(BEFORE);
    raw.extend_from_slice(&binding.finalize());
    raw.extend_from_slice(&[5, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]);
    raw.extend_from_slice(b"Items");
    raw.extend_from_slice(&7_u64.to_le_bytes());
    raw.extend_from_slice(&3_u32.to_le_bytes());
    raw.extend_from_slice(&[2, 0, 3]);
    raw.extend_from_slice(&1_u64.to_le_bytes());
    raw.extend_from_slice(&4_u32.to_le_bytes());
    raw.extend_from_slice(&[3, 0, 0, 7]);
    assert_eq!(raw.len(), 104);
    raw
}

#[test]
fn raw_order_roundtrip_and_source_binding_are_distinct_from_update_records() {
    let raw = literal();
    let decoded = NativeDeleteIntent::decode(&raw, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.table(), "Items");
    assert_eq!(
        decoded
            .deletes()
            .iter()
            .map(RowDelete::row)
            .collect::<Vec<_>>(),
        [7, 1]
    );
    assert_eq!(decoded.deletes()[0].expected_bytes(), &[2, 0, 3]);
    assert_eq!(decoded.deletes()[1].expected_bytes(), &[3, 0, 0, 7]);
    decoded
        .verify_before(BEFORE, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), raw);
    let intent = NativeDeleteIntent::new(
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
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), raw);
    assert!(matches!(
        intent.verify_before(b"modified raw source", &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
    let mut update = raw.clone();
    update[..8].copy_from_slice(b"TCNI0001");
    assert!(matches!(
        NativeDeleteIntent::decode(&update, &mut budget(), &|| false),
        Err(NativeIntentError::InvalidRecord)
    ));
    let mut future = raw;
    future[..8].copy_from_slice(b"TCND0002");
    assert!(matches!(
        NativeDeleteIntent::decode(&future, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));
}

#[test]
fn complete_malformed_frames_and_duplicate_ordinals_refuse_before_heap_copy() {
    let raw = literal();
    let mut variants = Vec::new();
    for at in [8, 12, 14, 56, 60, 64, 81] {
        let mut changed = raw.clone();
        changed[at] ^= 0x80;
        variants.push(changed);
    }
    let mut duplicate = raw.clone();
    duplicate[88..96].copy_from_slice(&7_u64.to_le_bytes());
    variants.push(duplicate);
    let mut invalid_utf8 = raw.clone();
    invalid_utf8[68] = 0xff;
    variants.push(invalid_utf8);
    for end in 0..raw.len() {
        variants.push(raw[..end].to_vec());
    }
    for changed in variants {
        let mut allowance = ResourceBudget::new(0, 0, 1_000_000);
        let error = NativeDeleteIntent::decode(&changed, &mut allowance, &|| false)
            .err()
            .expect("malformed frame must refuse before allocating");
        assert!(!matches!(
            error,
            NativeIntentError::Inspect(InspectError::Limit(limit))
                if limit.resource == Resource::Bytes
        ));
        assert_eq!(allowance.remaining(Resource::Bytes), 0);
    }
    assert!(matches!(
        NativeDeleteIntent::new(
            "Items",
            vec![
                RowDelete::new(1, vec![1, 0, 1]),
                RowDelete::new(1, vec![1, 0, 1])
            ],
            BEFORE,
            &mut budget(),
            &|| false
        ),
        Err(NativeIntentError::Request(EditError::Conflict))
    ));
}

#[test]
fn empty_selection_keeps_binding_and_decoding_needs_no_row_allowance() {
    let original =
        NativeDeleteIntent::new("Items", Vec::new(), BEFORE, &mut budget(), &|| false).unwrap();
    let raw = original.encode(&mut budget(), &|| false).unwrap();
    let decoded = NativeDeleteIntent::decode(&raw, &mut budget(), &|| false).unwrap();
    assert!(decoded.deletes().is_empty());
    decoded
        .verify_before(BEFORE, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), raw);
}

#[test]
fn exact_pools_and_entry_middle_terminal_cancellation_publish_no_partial_record() {
    let raw = literal();
    let mut measured = budget();
    let initial_bytes = measured.remaining(Resource::Bytes);
    let initial_steps = measured.remaining(Resource::Steps);
    let polls = Cell::new(0);
    NativeDeleteIntent::decode(&raw, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let bytes = initial_bytes - measured.remaining(Resource::Bytes);
    let steps = initial_steps - measured.remaining(Resource::Steps);
    NativeDeleteIntent::decode(&raw, &mut ResourceBudget::new(bytes, 0, steps), &|| false).unwrap();
    for (byte_limit, step_limit) in [(bytes - 1, steps), (bytes, steps - 1)] {
        assert!(
            NativeDeleteIntent::decode(
                &raw,
                &mut ResourceBudget::new(byte_limit, 0, step_limit),
                &|| false
            )
            .is_err()
        );
    }
    for stop in [1, polls.get() / 2, polls.get()] {
        let count = Cell::new(0);
        assert!(matches!(
            NativeDeleteIntent::decode(&raw, &mut budget(), &|| {
                count.set(count.get() + 1);
                count.get() == stop
            }),
            Err(NativeIntentError::Inspect(InspectError::Cancelled))
        ));
    }
}
