//! Literal TCAT framing is independent of its production encoder and template codec.
mod native;
mod retained;

use super::{AddTableIntentError, NativeAddTableIntent};
use crate::{CreateColumn, FirstTableDefinition, InspectError};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{canonical_add_table, canonical_repeated_add_table};

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
            0 => raw[7] = b'5',
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

#[test]
fn repeated_v2_has_disjoint_length_grammar_and_preserves_the_four_template_versions() {
    for existing in [2, 3, 6, 18] {
        let before = canonical_repeated_add_table::with_existing_tables(existing).before;
        for version in 1..=4 {
            // Wire capture remains data-only: the original declaration need not
            // acquire native name/capacity authority from this framing test.
            let intent =
                NativeAddTableIntent::new(definition(version), &before, &mut budget(), &|| false)
                    .unwrap();
            let actual = intent.encode(&mut budget(), &|| false).unwrap();
            let mut expected = literal(&before, version);
            expected[..8].copy_from_slice(b"TCAT0002");
            expected[12..14].copy_from_slice(&2_u16.to_le_bytes());
            expected[16..24].copy_from_slice(&(before.len() as u64).to_le_bytes());
            assert_eq!(actual, expected);
            let decoded =
                NativeAddTableIntent::decode(&expected, &mut budget(), &|| false).unwrap();
            decoded
                .verify_before(&before, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), expected);
            for at in [10 * 4096 + 3500, 11 * 4096 + 512, 12 * 4096 + 3500] {
                let mut changed = before.clone();
                changed[at] ^= 1;
                assert!(matches!(
                    decoded.verify_before(&changed, &mut budget(), &|| false),
                    Err(AddTableIntentError::BeforeMismatch)
                ));
            }
            let mut downgraded = actual;
            downgraded[..8].copy_from_slice(b"TCAT0001");
            downgraded[12..14].copy_from_slice(&1_u16.to_le_bytes());
            assert!(matches!(
                NativeAddTableIntent::decode(
                    &downgraded,
                    &mut ResourceBudget::new(0, 0, 10_000_000),
                    &|| false
                ),
                Err(AddTableIntentError::InvalidRecord)
            ));
        }
    }

    // Matching profile numbers alone never move source lengths across versions.
    let old = canonical_add_table::source().before;
    let mut widened = literal(&old, 1);
    widened[..8].copy_from_slice(b"TCAT0002");
    widened[12..14].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        NativeAddTableIntent::decode(
            &widened,
            &mut ResourceBudget::new(0, 0, 10_000_000),
            &|| false
        ),
        Err(AddTableIntentError::InvalidRecord)
    ));

    for length in [
        0,
        9 * 4096,
        11 * 4096,
        12 * 4096,
        13 * 4096 - 1,
        13 * 4096 + 1,
        14 * 4096,
        15 * 4096,
        62 * 4096,
        64 * 4096,
        67 * 4096,
    ] {
        let before = vec![0; length];
        assert!(
            matches!(
                NativeAddTableIntent::new(definition(1), &before, &mut budget(), &|| false),
                Err(AddTableIntentError::BeforeMismatch)
            ),
            "length {length}"
        );
        let mut raw = literal(&before, 1);
        raw[..8].copy_from_slice(b"TCAT0002");
        raw[12..14].copy_from_slice(&2_u16.to_le_bytes());
        raw[16..24].copy_from_slice(&(length as u64).to_le_bytes());
        assert!(
            matches!(
                NativeAddTableIntent::decode(
                    &raw,
                    &mut ResourceBudget::new(0, 0, 10_000_000),
                    &|| false
                ),
                Err(AddTableIntentError::InvalidRecord)
            ),
            "length {length}"
        );
    }
}

#[test]
fn repeated_v2_rejects_future_nested_templates_wrong_profiles_and_trailing_bytes_before_allocation()
{
    let before = canonical_repeated_add_table::source().before;
    let raw = NativeAddTableIntent::new(definition(1), &before, &mut budget(), &|| false)
        .unwrap()
        .encode(&mut budget(), &|| false)
        .unwrap();
    for case in 0..5 {
        let mut corrupt = raw.clone();
        match case {
            0 => corrupt[7] = b'6',
            1 => corrupt[12] = 1,
            2 => corrupt[64 + 7] = b'5',
            3 => corrupt[64 + 12] = 4,
            4 => corrupt.push(0),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                NativeAddTableIntent::decode(
                    &corrupt,
                    &mut ResourceBudget::new(0, 0, 10_000_000),
                    &|| false
                ),
                Err(AddTableIntentError::UnsupportedVersion
                    | AddTableIntentError::UnsupportedProfile
                    | AddTableIntentError::InvalidRecord)
            ),
            "case {case}"
        );
    }
    let mut none = ResourceBudget::new(0, 0, 10_000_000);
    assert!(NativeAddTableIntent::decode(&raw, &mut none, &|| false).is_err());
    assert!(matches!(
        NativeAddTableIntent::decode(&raw, &mut budget(), &|| true),
        Err(AddTableIntentError::Inspect(InspectError::Cancelled))
    ));
}

fn described_definition(text: &str) -> FirstTableDefinition {
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::short_text("Name", 20)
            .unwrap()
            .with_description(text)
            .unwrap(),
    ];
    FirstTableDefinition::capture("More", &columns, &mut budget(), &|| false).unwrap()
}

fn described_literal(before: &[u8], text: &str, repeated: bool) -> Vec<u8> {
    let mut nested = template(1);
    nested[..8].copy_from_slice(b"TCNT0005");
    nested[12..14].copy_from_slice(&5_u16.to_le_bytes());
    nested[90] = 32;
    nested.extend((text.len() as u16).to_le_bytes());
    nested.extend(text.as_bytes());
    let nested_length = nested.len() as u32;
    nested[8..12].copy_from_slice(&nested_length.to_le_bytes());
    let mut raw = literal(before, 1);
    raw.truncate(64);
    raw[..8].copy_from_slice(if repeated { b"TCAT0004" } else { b"TCAT0003" });
    raw[12..14].copy_from_slice(&(if repeated { 4_u16 } else { 3_u16 }).to_le_bytes());
    raw[16..24].copy_from_slice(&(before.len() as u64).to_le_bytes());
    raw[56..60].copy_from_slice(&nested_length.to_le_bytes());
    raw.extend(nested);
    let length = raw.len() as u32;
    raw[8..12].copy_from_slice(&length.to_le_bytes());
    raw
}

#[test]
fn described_outer_versions_keep_source_partitions_and_exact_original_template_text() {
    let boundary = "🗃".repeat(127) + "x";
    for repeated in [false, true] {
        let before = if repeated {
            canonical_repeated_add_table::source().before
        } else {
            canonical_add_table::source().before
        };
        for text in ["", "Résumé", boundary.as_str()] {
            let raw = NativeAddTableIntent::new(
                described_definition(text),
                &before,
                &mut budget(),
                &|| false,
            )
            .unwrap()
            .encode(&mut budget(), &|| false)
            .unwrap();
            let expected = described_literal(&before, text, repeated);
            assert_eq!(raw, expected);
            let decoded =
                NativeAddTableIntent::decode(&expected, &mut budget(), &|| false).unwrap();
            decoded
                .verify_before(&before, &mut budget(), &|| false)
                .unwrap();
            let columns = decoded
                .definition
                .borrowed_columns(&mut budget(), &|| false)
                .unwrap();
            assert_eq!(columns[0].description(), None);
            assert_eq!(columns[1].description(), Some(text));
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), expected);
        }
    }
}

#[test]
fn legacy_outer_languages_refuse_described_templates_and_described_outer_requires_tcnt5() {
    for repeated in [false, true] {
        let before = if repeated {
            canonical_repeated_add_table::source().before
        } else {
            canonical_add_table::source().before
        };
        let described = described_literal(&before, "", repeated);
        let mut legacy = described.clone();
        legacy[..8].copy_from_slice(if repeated { b"TCAT0002" } else { b"TCAT0001" });
        legacy[12..14].copy_from_slice(&(if repeated { 2_u16 } else { 1_u16 }).to_le_bytes());
        assert!(matches!(
            NativeAddTableIntent::decode(
                &legacy,
                &mut ResourceBudget::new(0, 0, 10_000_000),
                &|| false
            ),
            Err(AddTableIntentError::UnsupportedVersion)
        ));
        for case in 0..4 {
            let mut corrupt = described.clone();
            match case {
                0 => corrupt[12] = if repeated { 3 } else { 4 },
                1 => corrupt[16..24]
                    .copy_from_slice(&(if repeated { 40960_u64 } else { 53248_u64 }).to_le_bytes()),
                2 => corrupt[64 + 7] = b'4',
                3 => corrupt[64 + 12] = 4,
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    NativeAddTableIntent::decode(
                        &corrupt,
                        &mut ResourceBudget::new(0, 0, 10_000_000),
                        &|| false
                    ),
                    Err(AddTableIntentError::UnsupportedVersion
                        | AddTableIntentError::UnsupportedProfile
                        | AddTableIntentError::InvalidRecord)
                ),
                "case {case}"
            );
        }
    }
    let mut oversized_old = vec![0; 21_313];
    oversized_old[..8].copy_from_slice(b"TCAT0002");
    let mut entry_only = ResourceBudget::new(0, 0, 1);
    assert!(matches!(
        NativeAddTableIntent::decode(&oversized_old, &mut entry_only, &|| false),
        Err(AddTableIntentError::InvalidRecord)
    ));
    assert_eq!(entry_only.remaining(Resource::Steps), 0);
    assert_eq!(entry_only.remaining(Resource::Bytes), 0);
}

#[test]
fn all_existing_append_actions_refuse_manual_tcnt6_without_decoder_fallback() {
    let mut nested = template(1);
    nested[..8].copy_from_slice(b"TCNT0006");
    nested[12..14].copy_from_slice(&6_u16.to_le_bytes());
    nested[58..60].copy_from_slice(&0_u16.to_le_bytes());
    nested[62..64].copy_from_slice(&65535_u16.to_le_bytes());
    nested.drain(68..77);
    nested[70] = 4;
    let nested_length = nested.len() as u32;
    nested[8..12].copy_from_slice(&nested_length.to_le_bytes());
    crate::NativeFirstTableIntent::decode(&nested, &mut budget(), &|| false).unwrap();
    for version in 1..=4_u16 {
        let before = if version == 2 || version == 4 {
            canonical_repeated_add_table::source().before
        } else {
            canonical_add_table::source().before
        };
        let mut raw = literal(&before, 1);
        raw.truncate(64);
        raw[7] = b'0' + version as u8;
        raw[12..14].copy_from_slice(&version.to_le_bytes());
        raw[16..24].copy_from_slice(&(before.len() as u64).to_le_bytes());
        raw[56..60].copy_from_slice(&nested_length.to_le_bytes());
        raw.extend(&nested);
        let length = raw.len() as u32;
        raw[8..12].copy_from_slice(&length.to_le_bytes());
        let mut no_ownership = ResourceBudget::new(0, 0, 1_000_000);
        assert!(
            matches!(
                NativeAddTableIntent::decode(&raw, &mut no_ownership, &|| false),
                Err(AddTableIntentError::UnsupportedVersion)
            ),
            "outer version {version}"
        );
        assert_eq!(no_ownership.remaining(Resource::Bytes), 0);
    }
}

mod mixed;
