//! Independent numeric originals retain scalar, direction and allocated identities.
#[cfg(feature = "ace16-create")]
mod recovery;

use super::{NativeNumericIndexCreateIntent as Intent, NumericIndexCreateIntentError as Error};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
struct Facts {
    table: i32,
    definition: u32,
    field: u16,
    stored: u16,
    logical: u32,
    physical: u32,
    native_type: u8,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn literal(before: &[u8], names: [&str; 3], facts: Facts, descending: bool) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire = b"TCIC0003".to_vec();
    wire.extend(((88 + names.iter().map(|name| name.len()).sum::<usize>()) as u32).to_le_bytes());
    wire.extend(3_u16.to_le_bytes());
    wire.extend(u16::from(descending).to_le_bytes());
    wire.extend((before.len() as u64).to_le_bytes());
    wire.extend(digest.finalize());
    wire.extend(facts.table.to_le_bytes());
    wire.extend(facts.definition.to_le_bytes());
    wire.extend(facts.field.to_le_bytes());
    wire.extend(facts.stored.to_le_bytes());
    for name in names {
        wire.extend((name.len() as u16).to_le_bytes());
    }
    wire.extend(0_u16.to_le_bytes());
    wire.extend(facts.logical.to_le_bytes());
    wire.extend(facts.physical.to_le_bytes());
    wire.push(facts.native_type);
    wire.extend([0, 0, 0]);
    for name in names {
        wire.extend(name.as_bytes());
    }
    wire
}

fn facts(native_type: u8) -> Facts {
    Facts {
        table: -17,
        definition: 14,
        field: 0,
        stored: 0,
        logical: 1,
        physical: 1,
        native_type,
    }
}

#[test]
fn literal_numeric_tags_directions_signed_tables_and_zero_field_identities_roundtrip() {
    let before = vec![0; 20 * 4096];
    for (native, scalar) in [
        (1, ScalarType::Boolean),
        (2, ScalarType::Byte),
        (3, ScalarType::Int16),
        (4, ScalarType::Int32),
        (5, ScalarType::Currency),
        (6, ScalarType::Single),
        (7, ScalarType::Double),
        (8, ScalarType::DateTime),
        (19, ScalarType::Int64),
    ] {
        for descending in [false, true] {
            let wire = literal(
                &before,
                ["NumericValues", "KeyValue", "NumericLookup"],
                facts(native),
                descending,
            );
            let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
            assert_eq!(intent.table(), "NumericValues");
            assert_eq!(intent.column(), "KeyValue");
            assert_eq!(intent.index_name(), "NumericLookup");
            assert_eq!(intent.descending(), descending);
            assert_eq!(intent.native_table_id(), -17);
            assert_eq!(intent.definition_page(), 14);
            assert_eq!(intent.native_number(), 0);
            assert_eq!(intent.column_id(), 0);
            assert_eq!(intent.logical_index_number(), 1);
            assert_eq!(intent.physical_index_number(), 1);
            assert_eq!(intent.native_type(), native);
            assert_eq!(intent.scalar_type(), scalar);
            assert_eq!(intent.before_length(), before.len() as u64);
            assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
            intent
                .verify_before(&before, &mut budget(), &|| false)
                .unwrap();
            assert!(
                crate::NativeIndexCreateIntent::decode(&wire, &mut budget(), &|| false).is_err()
            );
            assert!(
                crate::NativeFreshIndexCreateIntent::decode(&wire, &mut budget(), &|| false)
                    .is_err()
            );
        }
    }
    for table in [i32::MIN, 0, i32::MAX] {
        let wire = literal(
            &before,
            ["NumericValues", "KeyValue", "NumericLookup"],
            Facts { table, ..facts(4) },
            false,
        );
        let data = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(data.native_table_id(), table);
        assert_eq!(data.encode(&mut budget(), &|| false).unwrap(), wire);
    }
    // Older frames are independently framed, not retagged full new headers.
    let wire = literal(
        &before,
        ["NumericValues", "KeyValue", "NumericLookup"],
        Facts {
            table: 14,
            ..facts(4)
        },
        false,
    );
    for (version, header) in [(1, 76), (2, 84)] {
        let mut old = wire[..header].to_vec();
        old[..8].copy_from_slice(if version == 1 {
            b"TCIC0001"
        } else {
            b"TCIC0002"
        });
        old[12..14].copy_from_slice(&(version as u16).to_le_bytes());
        old.extend_from_slice(&wire[88..]);
        let length = old.len() as u32;
        old[8..12].copy_from_slice(&length.to_le_bytes());
        if version == 1 {
            crate::NativeIndexCreateIntent::decode(&old, &mut budget(), &|| false).unwrap();
        } else {
            crate::NativeFreshIndexCreateIntent::decode(&old, &mut budget(), &|| false).unwrap();
        }
        assert!(Intent::decode(&old, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn numeric_framing_name_binding_and_resource_boundaries_refuse_without_fallback() {
    let before = vec![0; 20 * 4096];
    let wire = literal(
        &before,
        ["NumericValues", "KeyValue", "NumericLookup"],
        facts(4),
        false,
    );
    for end in 0..wire.len() {
        assert!(Intent::decode(&wire[..end], &mut budget(), &|| false).is_err());
    }
    for (offset, value) in [
        (0, b'X'),
        (7, b'2'),
        (8, 0),
        (12, 2),
        (14, 2),
        (60, 0),
        (68, 0),
        (70, 0),
        (72, 0),
        (74, 1),
        (75, 1),
        (85, 1),
        (86, 1),
        (87, 1),
    ] {
        let mut wrong = wire.to_vec();
        wrong[offset] = value;
        assert!(
            Intent::decode(&wrong, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
    for tag in [0, 9, 10, 15, 16, 20, 255] {
        let mut wrong = wire.to_vec();
        wrong[84] = tag;
        assert!(
            Intent::decode(&wrong, &mut budget(), &|| false).is_err(),
            "tag {tag}"
        );
    }
    let mut extra = wire.to_vec();
    extra.push(0);
    assert!(Intent::decode(&extra, &mut budget(), &|| false).is_err());
    let mut utf8 = wire.to_vec();
    utf8[88] = 255;
    assert!(Intent::decode(&utf8, &mut budget(), &|| false).is_err());
    for name in ["界".repeat(64), "🦀".repeat(32)] {
        let frame = literal(
            &before,
            [&name, "KeyValue", "NumericLookup"],
            facts(4),
            false,
        );
        let decoded = Intent::decode(&frame, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), frame);
    }
    for name in [
        "界".repeat(65),
        "🦀".repeat(33),
        "bad.name".into(),
        String::new(),
    ] {
        let frame = literal(
            &before,
            ["NumericValues", &name, "NumericLookup"],
            facts(4),
            false,
        );
        assert!(Intent::decode(&frame, &mut budget(), &|| false).is_err());
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
fn journal_only_numeric_data_refuses_native_qualification_before_target_effects() {
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    // Framed data is deliberately not represented as a qualified native source.
    let before = vec![0; 20 * 4096];
    let mut after = before.to_vec();
    after[7000] = 1;
    let wire = literal(
        &before,
        ["NumericValues", "KeyValue", "NumericLookup"],
        facts(7),
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
            .qualify_retained_recovery(checked, CatalogScope::new(32100), &mut budget(), &|| false),
        Err(Error::UnsupportedSource)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
