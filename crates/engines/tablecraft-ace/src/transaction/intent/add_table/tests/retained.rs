//! TCAT6 is independent source-bound data and never an old-family decoder retry.
mod recovery;

use super::template;
use crate::{
    AddTableIntentError, AddTableSourceProfile, CreateColumn, FirstTableDefinition, InspectError,
    NativeAddTableIntent, NativeImage,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        Case, readdition::Target,
    },
    canonical_retained_add_table::Target as Added,
    canonical_table_drop::current::{self as raw, History},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 1_000_000, 128 * 1024 * 1024)
}

fn sources() -> [Vec<u8>; 3] {
    let number = raw::source(Case::NumberPresent, Target::Number, true, History::Inserted);
    let text = raw::source(Case::MixedBinary, Target::ShortText, true, History::Deleted);
    let empty = raw::source(Case::IdOnlyText, Target::ShortText, false, History::Empty);
    [number.bytes, raw::after(&text), raw::after(&empty)]
}

fn definition(target: Added) -> FirstTableDefinition {
    let id = CreateColumn::new("Id", ScalarType::Int32)
        .with_primary_key("PrimaryId")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    let companion = match target {
        Added::GeneratedNumber => CreateColumn::new("Score", ScalarType::Int32),
        Added::GeneratedText => CreateColumn::short_text("Name", 255).unwrap(),
        _ => panic!("these wire cases retain generated targets"),
    };
    FirstTableDefinition::capture("More", &[id, companion], &mut budget(), &|| false).unwrap()
}

fn nested(target: Added) -> Vec<u8> {
    let mut bytes = template(1);
    match target {
        Added::GeneratedNumber => {
            bytes.truncate(87);
            bytes.extend([5, 0, 4, 0, 0, 0, 0, 0]);
            bytes.extend(b"Score");
        }
        Added::GeneratedText => bytes[91..93].copy_from_slice(&255_u16.to_le_bytes()),
        _ => panic!("these wire cases retain generated targets"),
    }
    let length = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&length.to_le_bytes());
    bytes
}

fn literal(before: &[u8], target: Added) -> Vec<u8> {
    let nested = nested(target);
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    let mut bytes = b"TCAT0006".to_vec();
    bytes.extend(((64 + nested.len()) as u32).to_le_bytes());
    bytes.extend([6, 0, 0, 0]);
    bytes.extend((before.len() as u64).to_le_bytes());
    bytes.extend(hash.finalize());
    bytes.extend((nested.len() as u32).to_le_bytes());
    bytes.extend([0; 4]);
    assert_eq!(bytes.len(), 64);
    bytes.extend(nested);
    bytes
}

fn image(before: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_for_table_append(
        &mut Cursor::new(before),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn decode(before: &[u8], target: Added) -> NativeAddTableIntent {
    NativeAddTableIntent::decode_retained(&literal(before, target), &mut budget(), &|| false)
        .unwrap()
}

#[test]
fn retained_literal_binds_actual_lengths_and_keeps_old_decoders_and_mixed_plans_closed() {
    for before in sources() {
        for target in [Added::GeneratedNumber, Added::GeneratedText] {
            let expected = literal(&before, target);
            let intent = NativeAddTableIntent::new_retained(
                definition(target),
                &before,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(intent.source_profile(), AddTableSourceProfile::Retained);
            assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), expected);
            let decoded = decode(&before, target);
            decoded
                .verify_before(&before, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), expected);
            assert!(matches!(
                NativeAddTableIntent::decode(&expected, &mut budget(), &|| false),
                Err(AddTableIntentError::UnsupportedVersion)
            ));
            let mut changed = before.clone();
            *changed.last_mut().unwrap() ^= 1;
            assert!(matches!(
                decoded.verify_before(&changed, &mut budget(), &|| false),
                Err(AddTableIntentError::BeforeMismatch)
            ));
            let old = NativeImage::capture(
                &mut Cursor::new(&before),
                CatalogScope::new(3041),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                old.plan_add_table_mixed(definition(target), &mut budget(), &|| false)
                    .is_err()
            );
        }
    }
}

#[test]
fn retained_envelope_rejects_bad_lengths_flags_versions_and_explicit_work_controls() {
    let before = sources().into_iter().next().unwrap();
    let valid = literal(&before, Added::GeneratedText);
    for length in [0, 1, 4095, 45057, 2_147_483_648 + 4096, u64::MAX] {
        let mut bytes = valid.clone();
        bytes[16..24].copy_from_slice(&length.to_le_bytes());
        let mut work = ResourceBudget::new(0, 0, 1_000_000);
        assert!(matches!(
            NativeAddTableIntent::decode_retained(&bytes, &mut work, &|| false),
            Err(AddTableIntentError::InvalidRecord)
        ));
        assert_eq!(work.remaining(Resource::Bytes), 0);
    }
    let mut maximum = valid.clone();
    maximum[16..24].copy_from_slice(&2_147_483_648_u64.to_le_bytes());
    assert!(NativeAddTableIntent::decode_retained(&maximum, &mut budget(), &|| false).is_ok());
    for case in 0..8 {
        let mut bytes = valid.clone();
        match case {
            0 => bytes[7] = b'7',
            1 => bytes[12] = 5,
            2 => bytes[14] = 1,
            3 => bytes[60] = 1,
            4 => bytes[64 + 7] = b'7',
            5 => bytes[64 + 12] = 2,
            6 => bytes.push(0),
            7 => bytes[56] ^= 1,
            _ => unreachable!(),
        }
        assert!(
            NativeAddTableIntent::decode_retained(
                &bytes,
                &mut ResourceBudget::new(0, 0, 1_000_000),
                &|| false
            )
            .is_err()
        );
    }
    for version in 1..=5 {
        let mut bytes = valid.clone();
        bytes[7] = b'0' + version;
        bytes[12] = version;
        assert!(matches!(
            NativeAddTableIntent::decode_retained(&bytes, &mut budget(), &|| false),
            Err(AddTableIntentError::UnsupportedVersion)
        ));
    }
    assert!(
        NativeAddTableIntent::decode_retained(
            &valid,
            &mut ResourceBudget::new(0, 0, 1_000_000),
            &|| false
        )
        .is_err()
    );
    assert!(matches!(
        NativeAddTableIntent::decode_retained(&valid, &mut budget(), &|| true),
        Err(AddTableIntentError::Inspect(InspectError::Cancelled))
    ));
}

#[test]
fn large_retained_before_hash_is_data_only_and_not_capped_by_old_image_quota() {
    let mut before = vec![0; 64 * 1024 * 1024 + 4096];
    before[64 * 1024 * 1024] = 91;
    let expected = literal(&before, Added::GeneratedNumber);
    let intent = NativeAddTableIntent::new_retained(
        definition(Added::GeneratedNumber),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), expected);
    let decoded =
        NativeAddTableIntent::decode_retained(&expected, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    before[64 * 1024 * 1024] ^= 1;
    assert!(matches!(
        decoded.verify_before(&before, &mut budget(), &|| false),
        Err(AddTableIntentError::BeforeMismatch)
    ));
    assert!(
        NativeAddTableIntent::new(
            definition(Added::GeneratedNumber),
            &before,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    // No native source or journal permission follows from a valid data hash.
    assert!(
        NativeImage::capture_for_table_append(
            &mut Cursor::new(before),
            CatalogScope::new(3042),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}
