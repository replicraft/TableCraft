//! Independently framed UNIQUE original data; decoding is not write authority.
#[cfg(feature = "ace16-create")]
mod recovery;

use super::NativeUniqueLongIndexCreateIntent as Intent;
use crate::FreshIndexCreateIntentError as Error;
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(2 * 1024 * 1024 * 1024, 2_000_000, 1_000_000_000)
}

fn literal(before: &[u8], names: [&str; 3], definition: u32, descending: bool) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    let mut bytes = b"TCIC0004".to_vec();
    bytes.extend(((84 + names.iter().map(|name| name.len()).sum::<usize>()) as u32).to_le_bytes());
    bytes.extend(4_u16.to_le_bytes());
    bytes.extend(u16::from(descending).to_le_bytes());
    bytes.extend((before.len() as u64).to_le_bytes());
    bytes.extend(hash.finalize());
    bytes.extend((definition as i32).to_le_bytes());
    bytes.extend(definition.to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    for name in names {
        bytes.extend((name.len() as u16).to_le_bytes());
    }
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    for name in names {
        bytes.extend(name.as_bytes());
    }
    bytes
}

#[test]
fn literal_unique_create_framing_preserves_identity_names_and_old_family_closure() {
    let before = vec![0; 20 * 4096];
    let wire = literal(
        &before,
        ["UniqueValues", "CopiedScore", "ValueUnique"],
        14,
        false,
    );
    assert_eq!(&wire[..8], b"TCIC0004");
    assert_eq!(&wire[12..16], &[4, 0, 0, 0]);
    let original = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(original.table(), "UniqueValues");
    assert_eq!(original.index_name(), "ValueUnique");
    assert_eq!(original.column(), "CopiedScore");
    assert_eq!(original.native_number(), 0);
    assert_eq!(original.column_id(), 0);
    assert!(!original.descending());
    assert_eq!(original.native_table_id(), 14);
    assert_eq!(original.definition_page(), 14);
    assert_eq!(original.logical_index_number(), 0);
    assert_eq!(original.physical_index_number(), 0);
    original
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(original.encode(&mut budget(), &|| false).unwrap(), wire);
    assert!(crate::NativeIndexCreateIntent::decode(&wire, &mut budget(), &|| false).is_err());
    assert!(crate::NativeFreshIndexCreateIntent::decode(&wire, &mut budget(), &|| false).is_err());
    assert!(
        crate::NativeNumericIndexCreateIntent::decode(&wire, &mut budget(), &|| false).is_err()
    );

    for signed in [i32::MIN, -17, 0, i32::MAX] {
        let mut other = wire.to_vec();
        other[56..60].copy_from_slice(&signed.to_le_bytes());
        let decoded = Intent::decode(&other, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.native_table_id(), signed);
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), other);
    }
}

#[test]
fn unique_create_original_refuses_invalid_frames_names_bindings_and_resources() {
    let before = vec![0; 20 * 4096];
    let wire = literal(
        &before,
        ["UniqueValues", "CopiedScore", "ValueUnique"],
        14,
        false,
    );
    for end in 0..wire.len() {
        assert!(
            Intent::decode(&wire[..end], &mut budget(), &|| false).is_err(),
            "truncated at {end}"
        );
    }
    for (at, byte) in [
        (0, b'X'),
        (8, 0),
        (16, 1),
        (7, b'2'),
        (12, 2),
        (14, 2),
        (60, 0),
        (68, 0),
        (70, 0),
        (72, 0),
        (74, 1),
        (75, 1),
    ] {
        let mut invalid = wire.to_vec();
        invalid[at] = byte;
        assert!(
            Intent::decode(&invalid, &mut budget(), &|| false).is_err(),
            "offset {at}"
        );
    }
    let mut malformed_text = wire.to_vec();
    malformed_text[84] = 0xff;
    assert!(Intent::decode(&malformed_text, &mut budget(), &|| false).is_err());
    let mut extra = wire.to_vec();
    extra.push(0);
    assert!(Intent::decode(&extra, &mut budget(), &|| false).is_err());
    for name in ["界".repeat(64), "🦀".repeat(32)] {
        let expected = literal(&before, [&name, "CopiedScore", "ValueUnique"], 14, false);
        let decoded = Intent::decode(&expected, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), expected);
    }
    for name in ["界".repeat(65), "bad.name".into(), String::new()] {
        let invalid = literal(&before, ["UniqueValues", "CopiedScore", &name], 14, false);
        assert!(Intent::decode(&invalid, &mut budget(), &|| false).is_err());
    }
    let original = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
    let mut stale = before.to_vec();
    stale[7000] ^= 1;
    assert!(matches!(
        original.verify_before(&stale, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));
    assert!(matches!(
        Intent::decode(&wire, &mut budget(), &|| true),
        Err(Error::Inspect(crate::InspectError::Cancelled))
    ));
    assert!(Intent::decode(&wire, &mut ResourceBudget::new(0, 0, 0), &|| false).is_err());
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_unique_create_data_refuses_native_recovery_before_target_effects() {
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;
    let before = vec![0; 20 * 4096];
    let mut after = before.to_vec();
    after[7000] = 1;
    let wire = literal(
        &before,
        ["UniqueValues", "CopiedScore", "ValueUnique"],
        14,
        false,
    );
    let original = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
    let mut target = MemoryStorage::new(before.to_vec());
    let mut log = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut log,
            &before,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut log, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        original.qualify_retained_recovery(
            checked,
            CatalogScope::new(31800),
            &mut budget(),
            &|| false
        ),
        Err(Error::UnsupportedSource)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
