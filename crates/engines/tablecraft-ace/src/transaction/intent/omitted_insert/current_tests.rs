//! Current-presence wire data is distinct from the older omission families.
use super::*;
use crate::InsertFieldValue;
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_testkit::fixtures::access::tcna2;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 0, 20_000_000)
}

fn fields() -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Supplied(Value::Int32(24)),
        InsertFieldValue::Omitted,
        InsertFieldValue::Supplied(Value::Null),
    ])]
}

fn literal() -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update(8_u64.to_le_bytes());
    digest.update(b"original");
    let mut wire = tcna2::wider_presence_wire(
        "Defaults",
        8,
        digest.finalize().into(),
        vec![vec![Some(Value::Int32(24)), None, Some(Value::Null)]],
    );
    wire[..8].copy_from_slice(b"TCNA0007");
    wire[12..14].copy_from_slice(&7_u16.to_le_bytes());
    assert_eq!(wire.len(), 112);
    wire
}

#[test]
fn current_presence_wire_matches_independent_tags_and_keeps_older_decoders_closed() {
    let intent = NativeOmittedInsertIntent::new_current(
        "Defaults",
        fields(),
        b"original",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let wire = literal();
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
    let decoded =
        NativeOmittedInsertIntent::decode_current(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.table(), "Defaults");
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
    assert!(!decoded.is_legacy_profile());
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    assert!(matches!(
        NativeOmittedInsertIntent::decode(&wire, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));
    for version in [2_u8, 6] {
        let mut older = wire.to_vec();
        older[7] = b'0' + version;
        older[12] = version;
        assert!(matches!(
            NativeOmittedInsertIntent::decode_current(&older, &mut budget(), &|| false),
            Err(NativeIntentError::UnsupportedVersion)
        ));
    }
}

#[test]
fn current_presence_preflight_refuses_incomplete_or_malformed_frames_before_heap_copy() {
    let wire = literal();
    let mut invalid = (0..wire.len())
        .map(|end| wire[..end].to_vec())
        .collect::<Vec<_>>();
    for (offset, value) in [
        (8, 0),
        (12, 6),
        (14, 1),
        (64, 1),
        (80, 4),
        (96, 8),
        (97, 1),
        (100, 1),
    ] {
        let mut changed = wire.to_vec();
        changed[offset] = value;
        invalid.push(changed);
    }
    for raw in invalid {
        let mut work = budget();
        let bytes = work.remaining(Resource::Bytes);
        assert!(NativeOmittedInsertIntent::decode_current(&raw, &mut work, &|| false).is_err());
        assert_eq!(work.remaining(Resource::Bytes), bytes);
    }
    for resource in [Resource::Bytes, Resource::Steps] {
        let mut work = match resource {
            Resource::Bytes => ResourceBudget::new(0, 0, 20_000_000),
            Resource::Steps => ResourceBudget::new(32 * 1024 * 1024, 0, 0),
            Resource::Rows => unreachable!(),
        };
        assert!(
            matches!(NativeOmittedInsertIntent::decode_current(&wire, &mut work, &|| false), Err(NativeIntentError::Inspect(crate::InspectError::Limit(error))) if error.resource == resource)
        );
    }
    assert!(matches!(
        NativeOmittedInsertIntent::decode_current(&wire, &mut budget(), &|| true),
        Err(NativeIntentError::Inspect(crate::InspectError::Cancelled))
    ));
}

#[test]
fn current_presence_data_has_no_inherited_sixteen_thousand_row_quota() {
    // This is an inert request-codec test, not qualification of a database write.
    let rows = (0..16_385)
        .map(|_| OmittedRowInsert::new(vec![InsertFieldValue::Omitted]))
        .collect();
    let intent = NativeOmittedInsertIntent::new_current(
        "Defaults",
        rows,
        b"original",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let wire = intent.encode(&mut budget(), &|| false).unwrap();
    assert_eq!(wire.len(), 68 + 8 + 16_385 * 16);
    let decoded =
        NativeOmittedInsertIntent::decode_current(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.rows().len(), 16_385);
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
}

#[test]
fn current_presence_variable_carriers_use_wire_lengths_without_widening_legacy_families() {
    for binary in [false, true] {
        let value = if binary {
            Value::Binary(vec![0xa5; 8193])
        } else {
            Value::Text("x".repeat(8193))
        };
        let intent = NativeOmittedInsertIntent::new_current(
            "Defaults",
            vec![OmittedRowInsert::new(vec![InsertFieldValue::Supplied(
                value,
            )])],
            b"original",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let wire = intent.encode(&mut budget(), &|| false).unwrap();
        let decoded =
            NativeOmittedInsertIntent::decode_current(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
        let InsertFieldValue::Supplied(value) = &decoded.rows()[0].values()[0] else {
            panic!("wire must retain supplied variable data");
        };
        assert!(
            matches!(value, Value::Text(text) if !binary && text.len() == 8193)
                || matches!(value, Value::Binary(bytes) if binary && bytes.len() == 8193 && bytes.iter().all(|byte| *byte == 0xa5))
        );
        let mut older = wire.to_vec();
        older[7] = b'6';
        older[12] = 6;
        assert!(matches!(
            NativeOmittedInsertIntent::decode(&older, &mut budget(), &|| false),
            Err(NativeIntentError::InvalidRecord)
        ));
    }
}

#[cfg(feature = "ace16-create")]
mod native_recovery;
