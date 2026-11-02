//! Independent source conversion/batch checks; no generated INSERT planner.
use crate::{
    IndexCheckStatus, NativeImage, check_table, check_table_constraints,
    inspect_table_index_counter, inspect_table_properties, read_table_index,
    table::mutation::autonumber_counter,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(263);

struct Profile {
    header: u8,
    bytes: usize,
    hash: &'static str,
    definition: usize,
    data: [usize; 2],
    roots: [usize; 2],
}

const PROFILES: [Profile; 2] = [
    Profile {
        header: 5,
        bytes: 409600,
        hash: "0356754e5fa7ea5ea9040e5112b17c6cec46faff388bda9f65d92d34a4864df5",
        definition: 89,
        data: [93, 95],
        roots: [91, 92],
    },
    Profile {
        header: 6,
        bytes: 479232,
        hash: "abf41a6e18375c84e0f812d682f26ed903145f586a2ee82b6311a215bf2d95d9",
        definition: 106,
        data: [110, 112],
        roots: [108, 109],
    },
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn record(values: [i32; 3], text: &str) -> Vec<u8> {
    let mut result = Vec::from(4_u16.to_le_bytes());
    for value in values {
        result.extend_from_slice(&value.to_le_bytes());
    }
    for unit in text.encode_utf16() {
        result.extend_from_slice(&unit.to_le_bytes());
    }
    let end = result.len() as u16;
    result.extend_from_slice(&end.to_le_bytes());
    result.extend_from_slice(&14_u16.to_le_bytes());
    result.extend_from_slice(&1_u16.to_le_bytes());
    result.push(15);
    result
}

fn rows(profile: &Profile, changed: bool) -> Vec<(u32, [i32; 3], String)> {
    let mut values = Vec::new();
    for identifier in 1..=9 {
        let page = if identifier <= 8 {
            profile.data[0]
        } else {
            profile.data[1]
        };
        let slot = if identifier <= 8 { identifier - 1 } else { 0 };
        values.push((
            ((page as u32) << 8) | slot as u32,
            [identifier, identifier - 5, (identifier - 1) % 3],
            "o".repeat(230),
        ));
    }
    if changed {
        values.push((
            ((profile.data[1] as u32) << 8) | 1,
            [138, 17, 7],
            "L".repeat(200),
        ));
        values.push((
            ((profile.data[0] as u32) << 8) | 8,
            [139, 18, 7],
            "new".into(),
        ));
    }
    values.sort_by_key(|(reference, _, _)| *reference);
    values
}

fn wire(values: &[(u32, [i32; 3], String)], column: usize) -> (Vec<u8>, Vec<u8>) {
    let mut keys: Vec<_> = values
        .iter()
        .map(|(reference, fields, _)| (fields[column], *reference))
        .collect();
    keys.sort_unstable();
    let mut bytes = Vec::new();
    let mut bitmap = vec![0; 453];
    for (value, reference) in keys {
        bytes.push(127);
        bytes.extend_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
        bytes.extend_from_slice(&reference.to_be_bytes());
        let end = bytes.len();
        bitmap[end / 8] |= 1 << (end % 8);
    }
    (bytes, bitmap)
}

fn literal(original: &[u8], profile: &Profile) -> (Vec<u8>, Vec<u8>) {
    let definition = profile.definition * 4096;
    assert_eq!(original[definition + 102], 3);
    assert_eq!(&original[definition + 20..definition + 24], &[0; 4]);
    let mut before = original.to_vec();
    before[definition + 102] = 7;
    before[definition + 20..definition + 24].copy_from_slice(&137_i32.to_le_bytes());
    for (offset, (old, new)) in original.iter().zip(&before).enumerate() {
        if offset != definition + 102 && !(definition + 20..definition + 24).contains(&offset) {
            assert_eq!(old, new);
        }
    }

    let mut after = before.clone();
    for (page, slot, start, end, count, free, values, label) in [
        (
            profile.data[1],
            1,
            3194,
            3615,
            2,
            3176,
            [138, 17, 7],
            "L".repeat(200),
        ),
        (
            profile.data[0],
            8,
            221,
            248,
            9,
            189,
            [139, 18, 7],
            "new".into(),
        ),
    ] {
        let raw = record(values, &label);
        assert_eq!(raw.len(), end - start);
        assert_eq!(start - 14 - 2 * count, free);
        let base = page * 4096;
        after[base + start..base + end].copy_from_slice(&raw);
        after[base + 14 + 2 * slot..base + 16 + 2 * slot]
            .copy_from_slice(&(start as u16).to_le_bytes());
        after[base + 12..base + 14].copy_from_slice(&(count as u16).to_le_bytes());
        after[base + 2..base + 4].copy_from_slice(&(free as u16).to_le_bytes());
    }
    after[definition + 16..definition + 20].copy_from_slice(&11_u32.to_le_bytes());
    after[definition + 20..definition + 24].copy_from_slice(&139_i32.to_le_bytes());
    for (physical, column, history) in [(0, 0, 11_u32), (1, 2, 4)] {
        let base = profile.roots[physical] * 4096;
        let (old_keys, old_bitmap) = wire(&rows(profile, false), column);
        assert_eq!(&before[base + 480..base + 561], old_keys);
        assert_eq!(&before[base + 27..base + 480], old_bitmap);
        let (keys, bitmap) = wire(&rows(profile, true), column);
        assert_eq!(keys.len(), 99);
        after[base + 2..base + 4].copy_from_slice(&3517_u16.to_le_bytes());
        after[base + 27..base + 480].copy_from_slice(&bitmap);
        after[base + 480..base + 579].copy_from_slice(&keys);
        let counter = definition + 67 + 12 * physical;
        after[counter..counter + 4].copy_from_slice(&history.to_le_bytes());
    }
    (before, after)
}

fn values(rows: &[(u32, [i32; 3], String)]) -> Vec<Vec<Value>> {
    rows.iter()
        .map(|(_, fields, label)| {
            vec![
                Value::Int32(fields[0]),
                Value::Int32(fields[1]),
                Value::Int32(fields[2]),
                Value::Text(label.clone()),
            ]
        })
        .collect()
}

#[test]
#[ignore = "requires four literal multi-index Increment source views; no matching producer"]
fn source_conversion_and_batch_preserve_full_source_and_all_physical_aliases() {
    let originals =
        PathBuf::from(std::env::var_os("TABLECRAFT_INCREMENT_MULTI_ORIGINAL_DIR").unwrap());
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_INCREMENT_MULTI_SOURCE_DIR").unwrap());
    assert!(originals.is_absolute() && directory.is_absolute());
    let mut views = Vec::new();
    for profile in &PROFILES {
        let original =
            fs::read(originals.join(format!("h{}-association-original.accdb", profile.header)))
                .unwrap();
        assert_eq!(original.len(), profile.bytes);
        assert_eq!(hash(&original), profile.hash);
        let (before, after) = literal(&original, profile);
        for (changed, expected, suffix) in [(false, before, "before"), (true, after, "batch")] {
            let raw = fs::read(directory.join(format!(
                "h{}-increment-multi-{suffix}.accdb",
                profile.header
            )))
            .unwrap();
            assert_eq!(raw, expected);
            views.push((profile, changed, raw));
        }
    }

    for (profile, changed, raw) in &views {
        let image =
            NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
        let source =
            super::super::capture_source(&image, "Items", &mut budget(), &|| false).unwrap();
        let expected = rows(profile, *changed);
        assert_eq!(source.table.rows(), values(&expected));
        assert_eq!(source.table.schema().columns().len(), 4);
        assert_eq!(source.table.schema().columns()[0].flags(), 7);
        assert_eq!(source.table.schema().columns()[0].extended_flags(), 0);
        assert_eq!(source.table.schema().columns()[0].native_type(), 4);
        assert_eq!(source.indexes.indexes().len(), 4);
        assert_eq!(source.records.len(), expected.len());
        let counter =
            autonumber_counter::capture(&image, source.table.schema(), 0, &mut budget(), &|| false)
                .unwrap();
        assert_eq!(counter.last, if *changed { 139 } else { 137 });
        assert_eq!(counter.offset, profile.definition * 4096 + 20);
        for (reference, fields, label) in &expected {
            let retained = source
                .records
                .iter()
                .find(|row| row.reference == *reference)
                .unwrap();
            assert_eq!(retained.data, record(*fields, label));
        }
        let properties =
            inspect_table_properties(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
        assert!(properties.raw_bytes().is_empty());
        assert!(properties.groups().is_empty());
        let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(checked.rows(), expected.len());
        assert_eq!(checked.indexes().len(), 4);
        for index in checked.indexes() {
            assert_eq!(index.status(), IndexCheckStatus::Checked);
            let physical = index.index().data_number() as usize;
            assert!(physical < 2);
            assert_eq!(index.index().root_page() as usize, profile.roots[physical]);
            let mut ordered = expected.clone();
            ordered.sort_by_key(|(reference, fields, _)| {
                (fields[if physical == 0 { 0 } else { 2 }], *reference)
            });
            assert_eq!(
                read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    index.index().name(),
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                values(&ordered)
            );
            assert_eq!(
                inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    index.index().name(),
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .value(),
                match (physical, *changed) {
                    (0, false) => 9,
                    (0, true) => 11,
                    (1, false) => 3,
                    (1, true) => 4,
                    _ => unreachable!(),
                }
            );
        }
        assert!(
            check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap()
            .relationships()
            .is_empty()
        );
    }
    assert_eq!(views.len(), 4);
}
