//! Original DROP data binds names and identities, never decoded retirement authority.
#[cfg(feature = "ace16-create")]
mod recovery;

use super::{IndexDropIntentError as Error, NativeIndexDropIntent as Intent};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(2 * 1024 * 1024 * 1024, 20_000_000, 16_100_000_000)
}

fn literal(
    before: &[u8],
    names: [&str; 2],
    definition: u32,
    logical: u32,
    physical: u32,
) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);

    let mut wire = b"TCID0001".to_vec();
    wire.extend(((80 + names.iter().map(|name| name.len()).sum::<usize>()) as u32).to_le_bytes());
    wire.extend(1_u16.to_le_bytes());
    wire.extend(0_u16.to_le_bytes());
    wire.extend((before.len() as u64).to_le_bytes());
    wire.extend(digest.finalize());
    wire.extend((definition as i32).to_le_bytes());
    wire.extend(definition.to_le_bytes());
    wire.extend(logical.to_le_bytes());
    wire.extend(physical.to_le_bytes());
    for name in names {
        wire.extend((name.len() as u16).to_le_bytes());
    }
    wire.extend(0_u32.to_le_bytes());
    for name in names {
        wire.extend(name.as_bytes());
    }
    wire
}

#[test]
fn literal_drop_original_preserves_signed_table_and_zero_or_sparse_index_identities() {
    let before = vec![0; 20 * 4096];
    for (logical, physical) in [(0, 0), (7, 2)] {
        let mut wire = literal(
            &before,
            ["CompanionValues", "ScoreLookup"],
            14,
            logical,
            physical,
        );
        wire[56..60].copy_from_slice(&(-17_i32).to_le_bytes());
        let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(intent.table(), "CompanionValues");
        assert_eq!(intent.index_name(), "ScoreLookup");
        assert_eq!(intent.native_table_id(), -17);
        assert_eq!(intent.definition_page(), 14);
        assert_eq!(intent.logical_index_number(), logical);
        assert_eq!(intent.physical_index_number(), physical);
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
        intent
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        assert!(crate::NativeIndexCreateIntent::decode(&wire, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn drop_framing_names_complete_binding_and_resource_refusals_remain_closed() {
    let before = vec![0; 20 * 4096];
    let wire = literal(&before, ["CompanionValues", "ScoreLookup"], 14, 0, 0);
    for end in 0..wire.len() {
        assert!(Intent::decode(&wire[..end], &mut budget(), &|| false).is_err());
    }
    for (offset, value) in [
        (0, b'X'),
        (7, b'2'),
        (8, 0),
        (12, 2),
        (14, 1),
        (15, 1),
        (60, 0),
        (72, 0),
        (74, 0),
        (76, 1),
        (79, 1),
    ] {
        let mut invalid = wire.to_vec();
        invalid[offset] = value;
        assert!(
            Intent::decode(&invalid, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
    let mut invalid_utf8 = wire.to_vec();
    invalid_utf8[80] = 255;
    assert!(Intent::decode(&invalid_utf8, &mut budget(), &|| false).is_err());

    let mut extra = wire.to_vec();
    extra.push(0);
    assert!(Intent::decode(&extra, &mut budget(), &|| false).is_err());
    for name in ["界".repeat(64), "🦀".repeat(32)] {
        let valid = literal(&before, [&name, "ScoreLookup"], 14, 0, 0);
        assert_eq!(
            Intent::decode(&valid, &mut budget(), &|| false)
                .unwrap()
                .encode(&mut budget(), &|| false)
                .unwrap(),
            valid
        );
    }
    for name in ["界".repeat(65), "bad.name".into(), String::new()] {
        assert!(
            Intent::decode(
                &literal(&before, ["CompanionValues", &name], 14, 0, 0),
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
fn journal_only_drop_data_refuses_native_recovery_before_target_effects() {
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    // Data framing does not pretend this synthetic image is a native source.
    let before = vec![0; 20 * 4096];
    let mut after = before.to_vec();
    after[7000] = 1;
    let intent = Intent::decode(
        &literal(&before, ["Items", "ScoreLookup"], 14, 0, 0),
        &mut budget(),
        &|| false,
    )
    .unwrap();
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
            .qualify_retained_recovery(checked, CatalogScope::new(30900), &mut budget(), &|| false),
        Err(Error::UnsupportedSource)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
