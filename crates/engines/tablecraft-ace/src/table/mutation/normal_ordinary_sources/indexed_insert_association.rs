//! Read-only cross-page source qualification; no INSERT planner or journal.
use crate::{
    IndexCheckStatus, NativeImage, check_table, inspect_table_index_counter, read_table,
    read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(176);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn values(id: i32) -> Vec<Value> {
    let (score, guard, text) = match id {
        10 => (17, 7, "L".repeat(200)),
        11 => (18, 7, "new".to_owned()),
        _ => (id - 5, (id - 1) % 3, "o".repeat(230)),
    };
    vec![
        Value::Int32(id),
        Value::Int32(score),
        Value::Int32(guard),
        Value::Text(text),
    ]
}

fn record(id: i32) -> Vec<u8> {
    let (score, guard, text) = match id {
        10 => (17_i32, 7_i32, "L".repeat(200)),
        11 => (18, 7, "new".to_owned()),
        _ => (id - 5, (id - 1) % 3, "o".repeat(230)),
    };
    let mut raw = vec![4, 0];
    for value in [id, score, guard] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    for unit in text.encode_utf16() {
        raw.extend_from_slice(&unit.to_le_bytes());
    }
    let end = raw.len() as u16;
    raw.extend_from_slice(&end.to_le_bytes());
    raw.extend_from_slice(&[14, 0, 1, 0, 15]);
    raw
}

fn reference(id: i32, early: u32, late: u32) -> u32 {
    let (page, slot) = match id {
        1..=8 => (early, (id - 1) as u32),
        9 => (late, 0),
        10 => (late, 1),
        11 => (early, 8),
        _ => panic!("Unlisted source row"),
    };
    (page << 8) | slot
}

fn literal(before: &[u8], definition: usize, early: u32, late: u32, roots: [usize; 2]) -> Vec<u8> {
    let mut after = before.to_vec();
    for (id, number, slot, start, end, count, free) in [
        (10, late, 1, 3194, 3615, 2_u16, 3176_u16),
        (11, early, 8, 221, 248, 9, 189),
    ] {
        let raw = record(id);
        assert_eq!(raw.len(), end - start);
        let base = number as usize * 4096;
        after[base + start..base + end].copy_from_slice(&raw);
        let at = base + 14 + 2 * slot;
        after[at..at + 2].copy_from_slice(&(start as u16).to_le_bytes());
        after[base + 12..base + 14].copy_from_slice(&count.to_le_bytes());
        after[base + 2..base + 4].copy_from_slice(&free.to_le_bytes());
    }
    after[definition * 4096 + 16..definition * 4096 + 20].copy_from_slice(&11_u32.to_le_bytes());
    for (physical, history) in [(0, 11_u32), (1, 4)] {
        let root = roots[physical] * 4096;
        let mut entries = (1..=11)
            .map(|id| {
                let key = if physical == 0 {
                    id
                } else if id >= 10 {
                    7
                } else {
                    (id - 1) % 3
                };
                (key, reference(id, early, late))
            })
            .collect::<Vec<_>>();
        entries.sort_unstable();
        let mut wire = Vec::new();
        let mut bitmap = vec![0_u8; 453];
        for (key, reference) in entries {
            wire.push(0x7f);
            wire.extend_from_slice(&((key as u32) ^ 0x8000_0000).to_be_bytes());
            wire.extend_from_slice(&reference.to_be_bytes());
            let end = wire.len();
            bitmap[end / 8] |= 1 << (end % 8);
        }
        assert_eq!(wire.len(), 99);
        after[root + 2..root + 4].copy_from_slice(&3517_u16.to_le_bytes());
        after[root + 27..root + 480].copy_from_slice(&bitmap);
        after[root + 480..root + 579].copy_from_slice(&wire);
        let at = definition * 4096 + 67 + 12 * physical;
        after[at..at + 4].copy_from_slice(&history.to_le_bytes());
    }
    after
}

#[test]
#[ignore = "requires four original/literal cross-page INSERT association source views"]
fn source_only_cross_page_rows_keys_aliases_and_complete_complement_match() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_INDEXED_INSERT_ASSOCIATION_SOURCE_DIR").unwrap(),
    );
    assert!(directory.is_absolute());
    let mut all = Vec::new();
    for (header, length, digest, definition, early, late, roots) in [
        (
            5,
            409600,
            "0356754e5fa7ea5ea9040e5112b17c6cec46faff388bda9f65d92d34a4864df5",
            89,
            93,
            95,
            [91, 92],
        ),
        (
            6,
            479232,
            "abf41a6e18375c84e0f812d682f26ed903145f586a2ee82b6311a215bf2d95d9",
            106,
            110,
            112,
            [108, 109],
        ),
    ] {
        let original =
            fs::read(directory.join(format!("h{header}-association-original.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-association-inserted.accdb"))).unwrap();
        assert_eq!(original.len(), length);
        assert_eq!(hash(&original), digest);
        assert_eq!(after, literal(&original, definition, early, late, roots));
        all.push((original, after, early, late));
    }

    for (original, after, early, late) in all {
        for (changed, raw) in [(false, &original), (true, &after)] {
            let ids = if changed {
                vec![1, 2, 3, 4, 5, 6, 7, 8, 11, 9, 10]
            } else {
                (1..=9).collect::<Vec<_>>()
            };
            let wanted = ids.iter().copied().map(values).collect::<Vec<_>>();
            let image =
                NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let source = crate::table::mutation::normal_ordinary::capture(
                &image,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(source.records.len(), ids.len());
            assert_eq!(source.table.rows(), wanted);
            let refs = ids
                .iter()
                .map(|id| reference(*id, early, late))
                .collect::<Vec<_>>();
            assert_eq!(
                source
                    .records
                    .iter()
                    .map(|row| row.reference)
                    .collect::<Vec<_>>(),
                refs
            );
            let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(checked.rows(), ids.len());
            assert_eq!(checked.indexes().len(), 4);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            let sequential =
                read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(sequential.rows(), wanted);
            for (name, physical) in [
                ("PrimaryID", 0),
                ("PrimaryAlias", 0),
                ("GroupOrder", 1),
                ("GroupAlias", 1),
            ] {
                let mut ordered = ids.clone();
                ordered.sort_by_key(|id| {
                    let key = if physical == 0 {
                        *id
                    } else if *id >= 10 {
                        7
                    } else {
                        (*id - 1) % 3
                    };
                    (key, reference(*id, early, late))
                });
                let rows = read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    rows.rows(),
                    ordered.into_iter().map(values).collect::<Vec<_>>()
                );
                let counter = inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(counter.physical_index(), physical);
                let history = match (physical, changed) {
                    (0, false) => 9,
                    (0, true) => 11,
                    (_, false) => 3,
                    (_, true) => 4,
                };
                assert_eq!(counter.value(), history);
            }
            assert_eq!(image.as_bytes(), raw.as_slice());
        }
    }
}
