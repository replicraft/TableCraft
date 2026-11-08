//! Literal TCAT framing is independent of its production encoder and template codec.
use super::{AddTableIntentError, NativeAddTableIntent};
use crate::{CreateColumn, FirstTableDefinition, InspectError};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_add_table;

fn budget() -> ResourceBudget {
    ResourceBudget::new(10_000_000, 100_000, 10_000_000)
}

fn definition(version: u8) -> FirstTableDefinition {
    let primary = CreateColumn::new("Id", ScalarType::Int32)
        .with_primary_key("PrimaryId")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    let companion = match version {
        1 => CreateColumn::short_text("Name", 20).unwrap(),
        2 => CreateColumn::new("Name", ScalarType::Int32)
            .with_default_literal(&Value::Int32(0))
            .unwrap(),
        3 => CreateColumn::new("Name", ScalarType::Int32).with_required(false),
        4 => CreateColumn::new("Name", ScalarType::Guid)
            .with_default_literal(&Value::Null)
            .unwrap(),
        _ => panic!("closed independent template cases"),
    };
    FirstTableDefinition::capture("More", &[primary, companion], &mut budget(), &|| false).unwrap()
}

fn template(version: u8) -> Vec<u8> {
    let mut raw = format!("TCNT000{version}").into_bytes();
    raw.extend([0; 4]);
    raw.extend(u16::from(version).to_le_bytes());
    raw.extend([0; 2]);
    raw.extend(28672_u64.to_le_bytes());
    raw.extend([
        0xb8, 0x41, 0xe3, 0x67, 0xe3, 0x88, 0x19, 0xa7, 0xd8, 0xc7, 0x41, 0xa2, 0x4c, 0x2f, 0xdc,
        0x7f, 0xe8, 0xdd, 0xf3, 0x7c, 0x20, 0x0a, 0x5d, 0xb9, 0xa3, 0xf3, 0x19, 0x35, 0xe6, 0x2a,
        0x2e, 0x77,
    ]);
    raw.extend([4, 0, 9, 0, 2, 0, 0, 0]);
    raw.extend(b"MorePrimaryId");
    raw.extend([2, 0, 0, 0, 0, 0, 0, 0]);
    raw.extend(b"Id");
    let field = match version {
        1 => [4, 0, 11, 0, 20, 0, 0, 0],
        2 => [4, 0, 4, 1, 0, 0, 0, 0],
        3 => [4, 0, 4, 2, 0, 0, 0, 0],
        4 => [4, 0, 10, 1, 0, 0, 0, 0],
        _ => panic!("closed independent template cases"),
    };
    raw.extend(field);
    raw.extend(b"Name");
    if version == 2 {
        raw.extend([2, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0]);
    } else if version == 4 {
        raw.extend([0; 8]);
    }
    let length = raw.len() as u32;
    raw[8..12].copy_from_slice(&length.to_le_bytes());
    raw
}

fn literal(before: &[u8], version: u8) -> Vec<u8> {
    let nested = template(version);
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut raw = Vec::from(*b"TCAT0001");
    raw.extend(((64 + nested.len()) as u32).to_le_bytes());
    raw.extend([1, 0, 0, 0]);
    raw.extend(40960_u64.to_le_bytes());
    raw.extend(digest.finalize());
    raw.extend((nested.len() as u32).to_le_bytes());
    raw.extend([0; 4]);
    assert_eq!(raw.len(), 64);
    raw.extend(nested);
    raw
}

#[test]
fn all_four_closed_templates_match_literal_and_bind_actual_populated_source() {
    let source = canonical_add_table::source();
    for version in 1..=4 {
        let intent =
            NativeAddTableIntent::new(definition(version), &source.before, &mut budget(), &|| {
                false
            })
            .unwrap();
        let expected = literal(&source.before, version);
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), expected);
        let decoded = NativeAddTableIntent::decode(&expected, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.table(), "More");
        assert_eq!(decoded.column_count(), 2);
        decoded
            .verify_before(&source.before, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), expected);

        for at in [
            3 * 4096 + 512,
            4 * 4096 + 512,
            7 * 4096 + 3500,
            8 * 4096 + 4090,
        ] {
            let mut wrong = source.before.clone();
            wrong[at] ^= 1;
            assert!(matches!(
                decoded.verify_before(&wrong, &mut budget(), &|| false),
                Err(AddTableIntentError::BeforeMismatch)
            ));
        }
        assert!(matches!(
            decoded.verify_before(&source.before[..28672], &mut budget(), &|| false),
            Err(AddTableIntentError::BeforeMismatch)
        ));
    }
}

#[test]
fn outer_framing_and_nested_version_pairs_refuse_before_decoded_allocation() {
    let before = canonical_add_table::source().before;
    let original = literal(&before, 1);
    for case in 0..11 {
        let mut raw = original.clone();
        match case {
            0 => raw[7] = b'2',
            1 => raw[12] = 2,
            2 => raw[14] = 1,
            3 => raw[16..24].copy_from_slice(&28672_u64.to_le_bytes()),
            4 => raw[56] ^= 1,
            5 => raw[60] = 1,
            6 => raw[64 + 7] = b'5',
            7 => raw[64 + 12] = 2,
            8 => raw[64 + 14] = 1,
            9 => raw[64 + 62] = 1,
            10 => raw.push(0),
            _ => unreachable!(),
        }
        let mut no_allocations = ResourceBudget::new(0, 0, 10_000_000);
        let result = NativeAddTableIntent::decode(&raw, &mut no_allocations, &|| false);
        assert!(!matches!(
            result,
            Ok(_) | Err(AddTableIntentError::Inspect(_))
        ));
        assert_eq!(no_allocations.remaining(Resource::Bytes), 0);
    }
    for length in 0..original.len() {
        assert!(
            NativeAddTableIntent::decode(&original[..length], &mut budget(), &|| false).is_err()
        );
    }
    let excessive = vec![0; NativeAddTableIntent::MAX_ENCODED_BYTES + 1];
    assert!(matches!(
        NativeAddTableIntent::decode(&excessive, &mut budget(), &|| false),
        Err(AddTableIntentError::InvalidRecord)
    ));
    assert!(matches!(
        NativeAddTableIntent::new(definition(1), &before[..28672], &mut budget(), &|| false),
        Err(AddTableIntentError::BeforeMismatch)
    ));
}

#[test]
fn source_capture_encoding_and_decoding_keep_work_allocation_and_cancel_limits() {
    let source = canonical_add_table::source();
    let intent =
        NativeAddTableIntent::new(definition(1), &source.before, &mut budget(), &|| false).unwrap();
    let raw = intent.encode(&mut budget(), &|| false).unwrap();
    for resource in [Resource::Bytes, Resource::Steps] {
        let mut measured = budget();
        NativeAddTableIntent::decode(&raw, &mut measured, &|| false).unwrap();
        let used = budget().remaining(resource) - measured.remaining(resource);
        let mut exact = match resource {
            Resource::Bytes => ResourceBudget::new(used, 100_000, 10_000_000),
            Resource::Steps => ResourceBudget::new(10_000_000, 100_000, used),
            _ => unreachable!(),
        };
        NativeAddTableIntent::decode(&raw, &mut exact, &|| false).unwrap();
        let mut short = match resource {
            Resource::Bytes => ResourceBudget::new(used - 1, 100_000, 10_000_000),
            Resource::Steps => ResourceBudget::new(10_000_000, 100_000, used - 1),
            _ => unreachable!(),
        };
        assert!(NativeAddTableIntent::decode(&raw, &mut short, &|| false).is_err());
    }
    assert!(
        intent
            .encode(&mut ResourceBudget::new(0, 0, 10_000_000), &|| false)
            .is_err()
    );
    assert!(matches!(
        NativeAddTableIntent::new(definition(1), &source.before, &mut budget(), &|| true),
        Err(AddTableIntentError::Inspect(InspectError::Cancelled))
    ));
    assert!(
        intent
            .verify_before(&source.before, &mut budget(), &|| true)
            .is_err()
    );
    assert!(intent.encode(&mut budget(), &|| true).is_err());
    let seen = Cell::new(0);
    NativeAddTableIntent::decode(&raw, &mut budget(), &|| {
        seen.set(seen.get() + 1);
        false
    })
    .unwrap();
    let calls = seen.get();
    for stop in 1..=calls {
        seen.set(0);
        let error = NativeAddTableIntent::decode(&raw, &mut budget(), &|| {
            seen.set(seen.get() + 1);
            seen.get() == stop
        })
        .err()
        .unwrap_or_else(|| panic!("decode ignored cancellation at poll {stop}/{calls}"));
        assert!(
            matches!(
                &error,
                AddTableIntentError::Inspect(InspectError::Cancelled)
                    | AddTableIntentError::Declaration(crate::FirstTableIntentError::Inspect(
                        InspectError::Cancelled
                    ))
                    | AddTableIntentError::Declaration(crate::FirstTableIntentError::Create(
                        crate::CreateDatabaseError::Inspect(InspectError::Cancelled)
                    ))
            ),
            "stop {stop}/{calls}, observed polls {}, typed error {error:?}",
            seen.get(),
        );
        assert_eq!(
            seen.get(),
            stop,
            "cancellation must stop at the requested poll"
        );
    }
}
