//! Six source-only views; no INSERT planner, journal or event execution.
use crate::table::mutation::normal_ordinary;
use crate::{
    IndexCheckStatus, NativeImage, check_table, check_table_constraints,
    inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, env, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(168);
const OLD: [[Option<i32>; 3]; 3] = [
    [Some(1), Some(-1), None],
    [Some(2), Some(0), Some(1)],
    [Some(3), Some(1), Some(2)],
];

const SOURCE_PINS: [(u8, &str, &str); 6] = [
    (
        5,
        "original",
        "f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b",
    ),
    (
        5,
        "single-null",
        "4997ebef2b58eabf6142c2785bcc8f57a226322329e8d028d926e6e20fa0842f",
    ),
    (
        5,
        "shared-present-batch",
        "9f629f2f43af20c6236490c4cddbdd64eae3ca5660b92b7b658222691136851d",
    ),
    (
        6,
        "original",
        "c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae",
    ),
    (
        6,
        "single-null",
        "d8af8c062e4669908c0bab89af46c7f4bf9eda3eb1f56c20404f313c5fa65b85",
    ),
    (
        6,
        "shared-present-batch",
        "e41537a9154eeca8fb37d0d221d24420a084866a065573c36e81068f7a78133a",
    ),
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

fn entry(value: Option<i32>, page: u32, slot: usize, descending: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    match value {
        Some(value) => {
            let biased = (value as u32) ^ 0x8000_0000;
            bytes.push(if descending { 128 } else { 127 });
            let encoded = if descending { !biased } else { biased };
            bytes.extend_from_slice(&encoded.to_be_bytes());
        }
        None => bytes.push(if descending { 255 } else { 0 }),
    }
    bytes.extend_from_slice(&((page << 8) | slot as u32).to_be_bytes());
    bytes
}

fn literal_after(
    before: &[u8],
    definition: usize,
    data: u32,
    roots: [usize; 2],
    additions: &[[Option<i32>; 3]],
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
        for &value in values {
            // Observed inactive NULL payload, separate from its logical value.
            let payload = value.unwrap_or_default();
            record.extend_from_slice(&payload.to_le_bytes());
        }
        record.push(if values[2].is_none() { 3 } else { 7 });
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
        if physical == 0 {
            keys.sort_unstable();
        } else {
            keys.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
        }
        let mut payload = Vec::new();
        let mut bitmap = vec![0; 453];
        for (value, slot) in keys {
            payload.extend_from_slice(&entry(value, data, slot, physical == 1));
            let end = payload.len();
            bitmap[end / 8] |= 1 << (end % 8);
        }
        let distinct = rows
            .iter()
            .map(|row| row[column])
            .collect::<BTreeSet<_>>()
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
fn nullable_desc_manual_indexed_insert_views_have_complete_alias_membership() {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_DESC_NULL_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let manifest = fs::read(directory.join("manifest.json")).unwrap();
    assert_eq!(
        hash(&manifest),
        "29766255e68f64a0ea63f3ca562d4ba916600fe29eb393020380179faa0d5432"
    );
    let mut originals = Vec::new();
    for (header, length, digest, definition, data, roots) in [
        (
            5,
            389120,
            "f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b",
            89,
            93,
            [91, 92],
        ),
        (
            6,
            458752,
            "c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae",
            106,
            110,
            [108, 109],
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-desc-null-insert-original.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(hash(&before), digest);
        originals.push((header, definition, data, roots, before));
    }

    for (header, definition, data, roots, before) in originals {
        for (id, additions) in [
            ("original", Vec::new()),
            ("single-null", vec![[Some(4), Some(17), None]]),
            (
                "shared-present-batch",
                vec![[Some(4), Some(17), None], [Some(5), Some(18), Some(1)]],
            ),
        ] {
            let bytes =
                fs::read(directory.join(format!("h{header}-desc-null-insert-{id}.accdb"))).unwrap();
            let expected_hash = SOURCE_PINS
                .iter()
                .find(|(source_header, recipe, _)| *source_header == header && *recipe == id)
                .unwrap()
                .2;
            assert_eq!(hash(&bytes), expected_hash);
            assert_eq!(
                bytes,
                literal_after(&before, definition, data, roots, &additions)
            );
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let source =
                normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).unwrap();
            assert_eq!(source.indexes.indexes().len(), 4);
            assert_eq!(source.records.len(), 3 + additions.len());

            let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(checked.indexes().len(), 4);
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
                .map(|row| {
                    row.iter()
                        .copied()
                        .map(|value| match value {
                            Some(value) => Value::Int32(value),
                            None => Value::Null,
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let snapshot = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(snapshot.rows(), wanted);

            for (name, physical, history) in [
                ("PrimaryID", 0, rows.len()),
                ("PrimaryAlias", 0, rows.len()),
                ("GroupOrder", 1, 3),
                ("GroupAlias", 1, 3),
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
                let order: Vec<usize> = if physical == 0 {
                    (0..rows.len()).collect()
                } else {
                    match rows.len() {
                        3 => vec![2, 1, 0],
                        4 => vec![2, 1, 0, 3],
                        5 => vec![2, 1, 4, 0, 3],
                        _ => unreachable!(),
                    }
                };
                let expected = order
                    .into_iter()
                    .map(|slot| wanted[slot].clone())
                    .collect::<Vec<_>>();
                assert_eq!(indexed.rows(), expected);
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
            let constraints = check_table_constraints(
                &mut image.reader(),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(constraints.rows(), rows.len());
            for (checked, (name, physical)) in checked.indexes().iter().zip([
                ("PrimaryID", 0),
                ("PrimaryAlias", 0),
                ("GroupOrder", 1),
                ("GroupAlias", 1),
            ]) {
                assert_eq!(checked.index().name(), name);
                assert_eq!(checked.index().data_number(), physical);
                assert_eq!(checked.index().columns()[0].descending(), physical == 1);
            }
            assert_eq!(image.as_bytes(), bytes);
        }
    }
}
