//! Six source-only views; no INSERT planner, journal or event execution.
use crate::{
    IndexCheckStatus, NativeImage, check_table, inspect_table_index_counter, read_table,
    read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(168);
const OLD: [[i32; 3]; 3] = [[1, -1, 0], [2, 0, 1], [3, 1, 2]];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn entry(value: i32, page: u32, slot: usize) -> Vec<u8> {
    let mut bytes = vec![0x7f];
    bytes.extend_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
    bytes.extend_from_slice(&((page << 8) | slot as u32).to_be_bytes());
    bytes
}

fn literal_after(
    before: &[u8],
    definition: usize,
    data: u32,
    roots: [usize; 2],
    additions: &[[i32; 3]],
) -> Vec<u8> {
    let mut after = before.to_vec();
    let td = definition * 4096;
    let base = data as usize * 4096;
    let mut rows = OLD.to_vec();
    rows.extend_from_slice(additions);
    let mut end = 4051;

    for (index, values) in additions.iter().enumerate() {
        let start = end - 15;
        let mut record = vec![3, 0];
        for value in values {
            record.extend_from_slice(&value.to_le_bytes());
        }
        record.push(7);
        after[base + start..base + end].copy_from_slice(&record);
        let slot = base + 14 + 2 * (3 + index);
        after[slot..slot + 2].copy_from_slice(&(start as u16).to_le_bytes());
        end = start;
    }
    let total = rows.len();
    after[base + 2..base + 4].copy_from_slice(&((end - 14 - 2 * total) as u16).to_le_bytes());
    after[base + 12..base + 14].copy_from_slice(&(total as u16).to_le_bytes());
    after[td + 16..td + 20].copy_from_slice(&(total as u32).to_le_bytes());

    for (physical, column) in [(0, 0), (1, 2)] {
        let root = roots[physical] * 4096;
        let mut keys = rows
            .iter()
            .enumerate()
            .map(|(slot, row)| (row[column], slot))
            .collect::<Vec<_>>();
        keys.sort_unstable();
        let mut payload = Vec::new();
        let mut bitmap = vec![0; 453];
        for (value, slot) in keys {
            payload.extend_from_slice(&entry(value, data, slot));
            let end = payload.len();
            bitmap[end / 8] |= 1 << (end % 8);
        }
        let distinct = rows
            .iter()
            .map(|row| row[column])
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        after[td + 67 + 12 * physical..td + 71 + 12 * physical]
            .copy_from_slice(&(distinct as u32).to_le_bytes());
        after[root + 2..root + 4]
            .copy_from_slice(&((4096 - 480 - payload.len()) as u16).to_le_bytes());
        after[root + 27..root + 480].copy_from_slice(&bitmap);
        after[root + 480..root + 480 + payload.len()].copy_from_slice(&payload);
    }
    after
}

#[test]
#[ignore = "requires six immutable source-modeled manual indexed INSERT views"]
fn original_only_manual_indexed_insert_views_have_complete_alias_membership() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_INDEXED_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut originals = Vec::new();
    for (header, length, digest, definition, data, roots) in [
        (
            5,
            405504,
            "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
            89,
            93,
            [91, 92],
        ),
        (
            6,
            475136,
            "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
            106,
            110,
            [108, 109],
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-indexed-insert-original.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(hash(&before), digest);
        originals.push((header, definition, data, roots, before));
    }

    for (header, definition, data, roots, before) in originals {
        for (id, additions) in [
            ("original", Vec::new()),
            ("single", vec![[4, 17, 3]]),
            ("shared-secondary-batch", vec![[4, 17, 3], [5, 18, 3]]),
        ] {
            let bytes =
                fs::read(directory.join(format!("h{header}-indexed-insert-{id}.accdb"))).unwrap();
            assert_eq!(
                bytes,
                literal_after(&before, definition, data, roots, &additions)
            );
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let source = crate::table::mutation::normal_ordinary::capture(
                &image,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(source.indexes.indexes().len(), 4);
            assert_eq!(source.records.len(), 3 + additions.len());

            let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            assert_eq!(checked.rows(), 3 + additions.len());
            let mut rows = OLD.to_vec();
            rows.extend_from_slice(&additions);
            let wanted = rows
                .iter()
                .map(|row| row.iter().copied().map(Value::Int32).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            let snapshot = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(snapshot.rows(), wanted);

            for (name, physical, history) in [
                ("PrimaryID", 0, rows.len()),
                ("PrimaryAlias", 0, rows.len()),
                ("GroupOrder", 1, if additions.is_empty() { 3 } else { 4 }),
                ("GroupAlias", 1, if additions.is_empty() { 3 } else { 4 }),
            ] {
                let indexed = read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(indexed.rows(), wanted);
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
                assert_eq!(counter.value() as usize, history);
            }
            assert_eq!(image.as_bytes(), bytes);
        }
    }
}
