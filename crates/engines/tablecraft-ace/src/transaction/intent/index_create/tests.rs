//! Literal original bytes do not inherit authority from the request encoder.
#[cfg(feature = "ace16-create")]
mod recovery;

use super::{IndexCreateIntentError, NativeIndexCreateIntent};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn literal(
    before: &[u8],
    names: [&str; 3],
    definition: u32,
    number: u16,
    descending: bool,
) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire = b"TCIC0001".to_vec();
    wire.extend(((76 + names.iter().map(|name| name.len()).sum::<usize>()) as u32).to_le_bytes());
    wire.extend(1_u16.to_le_bytes());
    wire.extend(u16::from(descending).to_le_bytes());
    wire.extend((before.len() as u64).to_le_bytes());
    wire.extend(digest.finalize());
    wire.extend((definition as i32).to_le_bytes());
    wire.extend(definition.to_le_bytes());
    wire.extend(number.to_le_bytes());
    wire.extend(number.to_le_bytes());
    for name in names {
        wire.extend((name.len() as u16).to_le_bytes());
    }
    wire.extend([0, 0]);
    for name in names {
        wire.extend(name.as_bytes());
    }
    wire
}

#[test]
fn literal_index_original_preserves_direction_native_zero_and_whole_source_binding() {
    let before = vec![0; 14 * 4096];
    for descending in [false, true] {
        let wire = literal(
            &before,
            ["Archive", "CopiedScore", "ScoreLookup"],
            11,
            0,
            descending,
        );
        let intent = NativeIndexCreateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(intent.table(), "Archive");
        assert_eq!(intent.column(), "CopiedScore");
        assert_eq!(intent.index_name(), "ScoreLookup");
        assert_eq!(intent.descending(), descending);
        assert_eq!(intent.native_table_id(), 11);
        assert_eq!(intent.definition_page(), 11);
        assert_eq!(intent.native_number(), 0);
        assert_eq!(intent.column_id(), 0);
        assert_eq!(intent.before_length(), before.len() as u64);
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
        intent
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        let mut stale = before.to_vec();
        stale[4096 + 3000] ^= 1;
        assert!(matches!(
            intent.verify_before(&stale, &mut budget(), &|| false),
            Err(IndexCreateIntentError::BeforeMismatch)
        ));
        assert!(matches!(
            intent.verify_before(&before[..4096], &mut budget(), &|| false),
            Err(IndexCreateIntentError::BeforeMismatch)
        ));
    }
}

#[test]
fn index_original_names_count_utf16_units_and_keep_the_native_geometry_envelope() {
    let before = vec![0; 14 * 4096];
    for name in ["界".repeat(64), "🦀".repeat(32)] {
        let wire = literal(&before, [&name, "CopiedScore", "Index"], 11, 0, false);
        let intent = NativeIndexCreateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(intent.table(), name);
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
    }
    for name in [
        "界".repeat(65),
        "🦀".repeat(33),
        "bad.name".into(),
        "".into(),
    ] {
        let wire = literal(&before, [&name, "CopiedScore", "Index"], 11, 0, false);
        assert!(NativeIndexCreateIntent::decode(&wire, &mut budget(), &|| false).is_err());
    }
    for length in [64_u64 * 1024 * 1024 + 4096, 2_u64 * 1024 * 1024 * 1024] {
        let mut wire = literal(&before, ["Archive", "CopiedScore", "Index"], 11, 0, false);
        wire[16..24].copy_from_slice(&length.to_le_bytes());
        let intent = NativeIndexCreateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(intent.before_length(), length);
    }
}

#[test]
fn index_original_framing_profile_and_resource_refusals_publish_no_request() {
    let before = vec![0; 14 * 4096];
    let wire = literal(&before, ["Archive", "CopiedScore", "Index"], 11, 0, false);
    for end in 0..wire.len() {
        assert!(NativeIndexCreateIntent::decode(&wire[..end], &mut budget(), &|| false).is_err());
    }
    for (offset, value) in [
        (0, b'X'),
        (7, b'2'),
        (8, 0),
        (12, 2),
        (14, 2),
        (56, 0),
        (60, 0),
        (68, 0),
        (74, 1),
        (76, 0xff),
    ] {
        let mut malformed = wire.to_vec();
        malformed[offset] = value;
        assert!(
            NativeIndexCreateIntent::decode(&malformed, &mut budget(), &|| false).is_err(),
            "offset={offset}"
        );
    }
    let mut trailing = wire.to_vec();
    trailing.push(0);
    assert!(NativeIndexCreateIntent::decode(&trailing, &mut budget(), &|| false).is_err());
    assert!(matches!(
        NativeIndexCreateIntent::decode(&wire, &mut budget(), &|| true),
        Err(IndexCreateIntentError::Inspect(
            crate::InspectError::Cancelled
        ))
    ));
    assert!(
        NativeIndexCreateIntent::decode(&wire, &mut ResourceBudget::new(0, 0, 100_000), &|| false)
            .is_err()
    );
    assert!(
        NativeIndexCreateIntent::decode(
            &wire,
            &mut ResourceBudget::new(100_000, 100_000, 0),
            &|| false
        )
        .is_err()
    );
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_decodes_original_but_recovery_refuses_before_target_effects() {
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    let before = vec![0; 14 * 4096];
    let mut after = before.to_vec();
    after[100] = 1;
    let wire = literal(&before, ["Archive", "CopiedScore", "Index"], 11, 0, false);
    let mut target = MemoryStorage::new(before.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal_file,
            &before,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    let intent = NativeIndexCreateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        intent
            .qualify_retained_recovery(checked, CatalogScope::new(30101), &mut budget(), &|| false),
        Err(IndexCreateIntentError::UnsupportedSource)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}

#[cfg(feature = "ace16-create")]
#[test]
fn original_capture_rejects_fresh_construction_even_when_count_based_bytes_are_identical() {
    use crate::{NativeImage, SecondaryIndexCreate};
    use std::io::Cursor;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, Population};
    let case = raw::source(Population::NullableDuplicate, false);
    let image = NativeImage::capture_retained(
        &mut Cursor::new(&case.before),
        CatalogScope::new(28613),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let fresh = image
        .prepare_retained_fresh_long_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, false),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(fresh.candidate_bytes().len(), case.expected.len());
    assert_eq!(
        fresh
            .candidate_bytes()
            .iter()
            .zip(&case.expected)
            .position(|(a, b)| a != b),
        None
    );
    assert!(matches!(
        NativeIndexCreateIntent::capture(&fresh, &mut budget(), &|| false),
        Err(IndexCreateIntentError::InvalidRecord)
    ));
}
