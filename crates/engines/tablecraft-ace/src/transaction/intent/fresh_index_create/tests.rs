//! Literal fresh-index originals preserve both new identity witnesses.
#[cfg(feature = "ace16-create")]
mod recovery;

use super::{FreshIndexCreateIntentError as Error, NativeFreshIndexCreateIntent as Intent};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}
fn literal(before: &[u8], names: [&str; 3], definition: u32, descending: bool) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire = b"TCIC0002".to_vec();
    wire.extend(((84 + names.iter().map(|name| name.len()).sum::<usize>()) as u32).to_le_bytes());
    wire.extend(2_u16.to_le_bytes());
    wire.extend(u16::from(descending).to_le_bytes());
    wire.extend((before.len() as u64).to_le_bytes());
    wire.extend(digest.finalize());
    wire.extend((definition as i32).to_le_bytes());
    wire.extend(definition.to_le_bytes());
    wire.extend(4_u16.to_le_bytes());
    wire.extend(4_u16.to_le_bytes());
    for name in names {
        wire.extend((name.len() as u16).to_le_bytes());
    }
    wire.extend(0_u16.to_le_bytes());
    wire.extend(3_u32.to_le_bytes());
    wire.extend(2_u32.to_le_bytes());
    for name in names {
        wire.extend(name.as_bytes());
    }
    wire
}

#[test]
fn literal_fresh_original_preserves_request_generated_table_and_nondense_new_identities() {
    let before = vec![0; 20 * 4096];
    for descending in [false, true] {
        let wire = literal(
            &before,
            ["CompanionValues", "OtherScore", "OtherLookup"],
            14,
            descending,
        );
        let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(intent.table(), "CompanionValues");
        assert_eq!(intent.column(), "OtherScore");
        assert_eq!(intent.index_name(), "OtherLookup");
        assert_eq!(intent.descending(), descending);
        assert_eq!(intent.native_table_id(), 14);
        assert_eq!(intent.definition_page(), 14);
        assert_eq!(intent.native_number(), 4);
        assert_eq!(intent.column_id(), 4);
        assert_eq!(intent.logical_index_number(), 3);
        assert_eq!(intent.physical_index_number(), 2);
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
        intent
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        for native_table in [i32::MIN, -17, 0, i32::MAX] {
            let mut signed = wire.to_vec();
            signed[56..60].copy_from_slice(&native_table.to_le_bytes());
            let data = Intent::decode(&signed, &mut budget(), &|| false).unwrap();
            assert_eq!(data.native_table_id(), native_table);
            assert_eq!(data.definition_page(), 14);
            assert_eq!(data.encode(&mut budget(), &|| false).unwrap(), signed);
        }
        assert!(crate::NativeIndexCreateIntent::decode(&wire, &mut budget(), &|| false).is_err());
        let mut old = wire[..76].to_vec();
        old[..8].copy_from_slice(b"TCIC0001");
        old[12..14].copy_from_slice(&1_u16.to_le_bytes());
        old.extend_from_slice(&wire[84..]);
        let length = old.len() as u32;
        old[8..12].copy_from_slice(&length.to_le_bytes());
        crate::NativeIndexCreateIntent::decode(&old, &mut budget(), &|| false).unwrap();
        assert!(Intent::decode(&old, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn fresh_original_framing_names_binding_and_resource_controls_remain_closed() {
    let before = vec![0; 20 * 4096];
    let wire = literal(
        &before,
        ["CompanionValues", "OtherScore", "OtherLookup"],
        14,
        false,
    );
    for end in 0..wire.len() {
        assert!(Intent::decode(&wire[..end], &mut budget(), &|| false).is_err());
    }
    for (offset, value) in [
        (0, b'X'),
        (7, b'1'),
        (8, 0),
        (12, 1),
        (14, 2),
        (60, 0),
        (68, 0),
        (70, 0),
        (72, 0),
        (74, 1),
        (75, 1),
    ] {
        let mut invalid = wire.to_vec();
        invalid[offset] = value;
        assert!(
            Intent::decode(&invalid, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
    let mut extra = wire.to_vec();
    extra.push(0);
    assert!(Intent::decode(&extra, &mut budget(), &|| false).is_err());
    for name in ["界".repeat(64), "🦀".repeat(32)] {
        let wire = literal(&before, [&name, "OtherScore", "OtherLookup"], 14, false);
        assert_eq!(
            Intent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .encode(&mut budget(), &|| false)
                .unwrap(),
            wire
        );
    }
    for name in ["界".repeat(65), "bad.name".into(), String::new()] {
        assert!(
            Intent::decode(
                &literal(
                    &before,
                    ["CompanionValues", &name, "OtherLookup"],
                    14,
                    false
                ),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
    let mut stale = before.to_vec();
    stale[7000] ^= 1;
    assert!(matches!(
        intent.verify_before(&stale, &mut budget(), &|| false),
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
fn journal_only_fresh_original_data_cannot_grant_native_recovery_effects() {
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;
    let before = vec![0; 20 * 4096];
    let mut after = before.to_vec();
    after[7000] = 1;
    let wire = literal(
        &before,
        ["CompanionValues", "OtherScore", "OtherLookup"],
        14,
        false,
    );
    let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
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
    assert!(matches!(
        intent
            .qualify_retained_recovery(checked, CatalogScope::new(30800), &mut budget(), &|| false),
        Err(Error::UnsupportedSource)
    ));
    assert_eq!(target.writes, 0);
    assert!(target.bytes.get_ref() == &before);
}
