//! Original/request-derived DATA and key overlays, read-only native qualification.
use crate::{
    EditError, IndexCheckStatus, NativeImage, WriteBlocker, check_table, check_table_constraints,
    inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(196);
const ALIASES: [&str; 6] = [
    "PrimaryID",
    "PrimaryAlias",
    "Ascending",
    "AscendingAlias",
    "Descending",
    "DescendingAlias",
];
type Row = (usize, i32, Option<i64>, i32);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn rows(family: &str, recipe: &str) -> Vec<Row> {
    let mut rows = match family {
        "empty" => Vec::new(),
        "all-null" => vec![(0, 1, None, 17), (1, 2, None, 34), (2, 3, None, 51)],
        "crossing-byte" | "original" => vec![
            (0, 1, Some(i64::MIN), 17),
            (1, 2, None, 34),
            (2, 3, Some(i64::MAX), 51),
        ],
        _ => panic!("unknown source family"),
    };
    match recipe {
        "before" => {}
        "insert-null" => rows.push((3, 4, None, 68)),
        "insert-zero" => rows.push((3, 4, Some(0), 68)),
        "delete-all" => rows.clear(),
        _ => panic!("unknown original request"),
    }
    rows
}

fn values(table: &str, row: Row) -> Vec<Value> {
    let key = row
        .2
        .map(|key| {
            if table == "CurrencyKeys" {
                Value::Currency(Currency::from_scaled(key))
            } else {
                Value::Int64(key)
            }
        })
        .unwrap_or(Value::Null);
    vec![Value::Int32(row.1), key, Value::Int32(row.3)]
}

fn history(family: &str, recipe: &str, physical: usize) -> u32 {
    if !recipe.starts_with("insert-") {
        return 3;
    }
    if physical == 0 || family == "empty" || recipe == "insert-zero" {
        4
    } else {
        3
    }
}

fn key(row: Row, physical: usize) -> Option<i64> {
    if physical == 0 {
        Some(i64::from(row.1))
    } else {
        row.2
    }
}

fn order(rows: &mut [Row], physical: usize) {
    rows.sort_by(|left, right| {
        let comparison = key(*left, physical).cmp(&key(*right, physical));
        let comparison = if physical == 2 {
            comparison.reverse()
        } else {
            comparison
        };
        if comparison == Ordering::Equal {
            left.0.cmp(&right.0)
        } else {
            comparison
        }
    });
}

fn wire(row: Row, data: u32, physical: usize) -> Vec<u8> {
    let mut result = Vec::new();
    match key(row, physical) {
        None => result.push(if physical == 2 { 255 } else { 0 }),
        Some(value) => {
            result.push(if physical == 2 { 128 } else { 127 });
            let mut payload = if physical == 0 {
                ((value as i32 as u32) ^ 0x8000_0000).to_be_bytes().to_vec()
            } else {
                ((value as u64) ^ 0x8000_0000_0000_0000)
                    .to_be_bytes()
                    .to_vec()
            };
            if physical == 2 {
                payload.iter_mut().for_each(|byte| *byte = !*byte);
            }
            result.extend_from_slice(&payload);
        }
    }
    result.extend_from_slice(&((data << 8) | row.0 as u32).to_be_bytes());
    result
}

fn leaves(before: &[u8], after: &mut [u8], data: u32, roots: [u32; 3], states: &[Row]) {
    for (physical, root) in roots.iter().enumerate() {
        let base = *root as usize * 4096;
        let mut ordered = states.to_vec();
        order(&mut ordered, physical);
        let mut bitmap = [0_u8; 453];
        let mut active = Vec::new();
        for row in ordered {
            active.extend_from_slice(&wire(row, data, physical));
            bitmap[active.len() / 8] |= 1 << (active.len() % 8);
        }
        after[base + 27..base + 480].copy_from_slice(&bitmap);
        after[base + 480..base + 480 + active.len()].copy_from_slice(&active);
        after[base + 2..base + 4].copy_from_slice(&((3616 - active.len()) as u16).to_le_bytes());
        assert_eq!(
            &after[base + 480 + active.len()..base + 4096],
            &before[base + 480 + active.len()..base + 4096]
        );
    }
}

fn literal_before(
    original: &[u8],
    definition: u32,
    data: u32,
    _roots: [u32; 3],
    family: &str,
) -> Vec<u8> {
    assert_eq!(family, "crossing-byte");
    let mut source = original.to_vec();
    let td = definition as usize * 4096;
    let page = data as usize * 4096;
    source[td + 41..td + 43].copy_from_slice(&10_u16.to_le_bytes());
    for (position, native) in [(1, 8_u16), (2, 9_u16)] {
        let at = td + 99 + 25 * position;
        source[at + 5..at + 7].copy_from_slice(&native.to_le_bytes());
        source[at + 9..at + 11].copy_from_slice(&native.to_le_bytes());
    }
    for physical in [1, 2] {
        let at = td + 212 + 52 * physical + 4;
        source[at..at + 2].copy_from_slice(&8_u16.to_le_bytes());
    }
    for slot in 0..3 {
        let old = page + 4077 - slot * 19;
        let start = page + 4076 - slot * 20;
        let mut record = 10_u16.to_le_bytes().to_vec();
        record.extend_from_slice(&original[old + 2..old + 18]);
        let mask = if slot == 1 { 0x0201_u16 } else { 0x0301_u16 };
        record.extend_from_slice(&mask.to_le_bytes());
        assert_eq!(record.len(), 20);
        source[start..start + 20].copy_from_slice(&record);
        source[page + 14 + 2 * slot..page + 16 + 2 * slot]
            .copy_from_slice(&((4076 - slot * 20) as u16).to_le_bytes());
    }
    source[page + 2..page + 4].copy_from_slice(&4016_u16.to_le_bytes());
    source
}

fn literal_after(
    source: &[u8],
    definition: u32,
    data: u32,
    roots: [u32; 3],
    family: &str,
    recipe: &str,
) -> Vec<u8> {
    let mut after = source.to_vec();
    if recipe == "before" {
        return after;
    }
    let td = definition as usize * 4096;
    let page = data as usize * 4096;
    let states = rows(family, recipe);
    if recipe.starts_with("insert-") {
        let mut record = 10_u16.to_le_bytes().to_vec();
        record.extend_from_slice(&4_i32.to_le_bytes());
        record.extend_from_slice(&0_i64.to_le_bytes());
        record.extend_from_slice(&68_i32.to_le_bytes());
        record.extend_from_slice(
            &(if recipe == "insert-null" {
                0x0201_u16
            } else {
                0x0301_u16
            })
            .to_le_bytes(),
        );
        assert_eq!(record.len(), 20);
        after[page + 4016..page + 4036].copy_from_slice(&record);
        after[page + 20..page + 22].copy_from_slice(&4016_u16.to_le_bytes());
        after[page + 12..page + 14].copy_from_slice(&4_u16.to_le_bytes());
        after[page + 2..page + 4].copy_from_slice(&3994_u16.to_le_bytes());
    } else {
        assert_eq!(recipe, "delete-all");
        for slot in 0..3 {
            after[page + 14 + slot * 2..page + 16 + slot * 2]
                .copy_from_slice(&((4076_u16 - slot as u16 * 20) | 0xc000).to_le_bytes());
        }
    }
    after[td + 16..td + 20].copy_from_slice(&(states.len() as u32).to_le_bytes());
    leaves(source, &mut after, data, roots, &states);
    for physical in 0..3 {
        after[td + 67 + 12 * physical..td + 71 + 12 * physical]
            .copy_from_slice(&history(family, recipe, physical).to_le_bytes());
    }
    after
}

fn read_view(raw: &[u8], header: u8, selected: &str, family: &str, recipe: &str) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    for table in ["Int64Keys", "CurrencyKeys"] {
        let definition = match (header, table) {
            (5, "Int64Keys") => 89,
            (5, _) => 96,
            (6, "Int64Keys") => 106,
            (6, _) => 113,
            _ => unreachable!(),
        };
        let sparse = table == selected && family == "crossing-byte";
        let active = if table == selected {
            rows(family, recipe)
        } else {
            rows("original", "before")
        };
        let expected = active
            .iter()
            .map(|row| values(table, *row))
            .collect::<Vec<_>>();
        let snapshot =
            read_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
        assert_eq!(snapshot.rows(), expected);
        assert_eq!(snapshot.schema().definition_page(), definition);
        assert_eq!(
            snapshot.schema().layout.max_columns,
            if sparse { 10 } else { 3 }
        );
        for (position, field) in snapshot.schema().columns().iter().enumerate() {
            assert_eq!(
                field.number(),
                if sparse {
                    [0, 8, 9][position]
                } else {
                    position
                }
            );
        }
        let constraints =
            check_table_constraints(&mut image.reader(), SCOPE, table, &mut budget(), &|| false)
                .unwrap();
        assert_eq!(constraints.rows(), active.len());
        assert!(constraints.relationships().is_empty());
        let checked =
            check_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
        assert_eq!(checked.rows(), active.len());
        assert_eq!(checked.indexes().len(), 6);
        for (position, checked) in checked.indexes().iter().enumerate() {
            let physical = position / 2;
            let index = checked.index();
            assert_eq!(checked.status(), IndexCheckStatus::Checked);
            assert_eq!(index.name(), ALIASES[position]);
            assert_eq!(index.root_page(), definition + 2 + physical as u32);
            assert_eq!(
                index.columns()[0].number(),
                if physical == 0 {
                    0
                } else if sparse {
                    8
                } else {
                    1
                }
            );
            let stored = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                table,
                index.name(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                stored.value(),
                if table == selected {
                    history(family, recipe, physical)
                } else {
                    3
                }
            );
            let mut sorted = active.clone();
            order(&mut sorted, physical);
            let ordered = read_table_index(
                &mut image.reader(),
                SCOPE,
                table,
                index.name(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                ordered.rows(),
                sorted
                    .into_iter()
                    .map(|row| values(table, row))
                    .collect::<Vec<_>>()
            );
        }
        if sparse {
            let refused =
                super::super::normal_ordinary::capture(&image, table, &mut budget(), &|| false)
                    .err()
                    .unwrap();
            assert_eq!(refused, EditError::Unsupported(WriteBlocker::ActiveContent));
        } else {
            super::super::normal_ordinary::capture(&image, table, &mut budget(), &|| false)
                .unwrap();
            image
                .check_native_ordinary_fixed_table(table, &mut budget(), &|| false)
                .unwrap();
        }
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires16 crossing-byte source views and128 exact SDK reads"]
fn cross_byte_native_ordinals_have_exact_masks_rows_and_complete_membership() {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_CROSS_BYTE_SOURCE_DIR").unwrap());
    let old = PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_INDEXED_SOURCE_DIR").unwrap());
    assert!(dir.is_absolute() && old.is_absolute());
    assert_eq!(
        digest(&fs::read(dir.join("manifest.json")).unwrap()),
        "b920105bb4221aa84a578c8e922baa9ee2463820e89201a0f6be0052232af5c3"
    );
    let originals = [
        fs::read(old.join("h5-integral-original.accdb")).unwrap(),
        fs::read(old.join("h6-integral-original.accdb")).unwrap(),
    ];
    assert_eq!(
        digest(&originals[0]),
        "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1"
    );
    assert_eq!(
        digest(&originals[1]),
        "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e"
    );
    for (header, table, family, recipe, filename, hash) in [
        (
            5_u8,
            "Int64Keys",
            "crossing-byte",
            "before",
            "h5-Int64Keys-crossing-byte-before.accdb",
            "a87b51409365f249ef9143cfd7b74d4c423ddbc20e9927244e0d925ba2b72dee",
        ),
        (
            5_u8,
            "Int64Keys",
            "crossing-byte",
            "insert-null",
            "h5-Int64Keys-crossing-byte-insert-null.accdb",
            "f52900c273cdcaca344903ab839cd7487b7aaaceeebe636123bf62a679aaf7e4",
        ),
        (
            5_u8,
            "Int64Keys",
            "crossing-byte",
            "insert-zero",
            "h5-Int64Keys-crossing-byte-insert-zero.accdb",
            "7168bf5e0bd5e963eaa3547496d30d66c1ef37740950eaff4470171e7a5edd63",
        ),
        (
            5_u8,
            "Int64Keys",
            "crossing-byte",
            "delete-all",
            "h5-Int64Keys-crossing-byte-delete-all.accdb",
            "4f4d2e7618793ae20f16bce05a03018ca3729d24c5664b92418544a83d170d2e",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "crossing-byte",
            "before",
            "h5-CurrencyKeys-crossing-byte-before.accdb",
            "fabd24e78c5b547451d23bbf852b2a4d4f0fa1909c9dc2bfb3dd29caf526ba74",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "crossing-byte",
            "insert-null",
            "h5-CurrencyKeys-crossing-byte-insert-null.accdb",
            "0c812fa64b88a2bc2c2d54eddc8247ad96778e4d36dab0bfea9dc2e6c26ad3c8",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "crossing-byte",
            "insert-zero",
            "h5-CurrencyKeys-crossing-byte-insert-zero.accdb",
            "72fbeff20b068f5101da69c472c1c0e47a5803991168b4b20509f6534649ebc9",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "crossing-byte",
            "delete-all",
            "h5-CurrencyKeys-crossing-byte-delete-all.accdb",
            "c3e09219fc24bd163e45cc356cdc79bec46af3e1da2a952ef21790ccb8431c1e",
        ),
        (
            6_u8,
            "Int64Keys",
            "crossing-byte",
            "before",
            "h6-Int64Keys-crossing-byte-before.accdb",
            "f012f483886d5258915f45420f1b77a056f0ec29f019047c73289829c4baec42",
        ),
        (
            6_u8,
            "Int64Keys",
            "crossing-byte",
            "insert-null",
            "h6-Int64Keys-crossing-byte-insert-null.accdb",
            "d23ec368894741b0cd563fb5d2232da902493db4e1ace367be0469abbed067bf",
        ),
        (
            6_u8,
            "Int64Keys",
            "crossing-byte",
            "insert-zero",
            "h6-Int64Keys-crossing-byte-insert-zero.accdb",
            "2f53c1c611bf58ba91ae6db92b02ecaf064cebbe3a3ca2e4bc715227bb139c7c",
        ),
        (
            6_u8,
            "Int64Keys",
            "crossing-byte",
            "delete-all",
            "h6-Int64Keys-crossing-byte-delete-all.accdb",
            "306275731861b6162cdf206ab2e10a9254e4e607c244e7bd9b4d8d96c0ee4ebe",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "crossing-byte",
            "before",
            "h6-CurrencyKeys-crossing-byte-before.accdb",
            "1ada867caaa10aabb76f93f60ac906e0872bd5221f43920331f15cd6ed3bf13b",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "crossing-byte",
            "insert-null",
            "h6-CurrencyKeys-crossing-byte-insert-null.accdb",
            "6bbd07f72d0770e3f7b1b64850a4020cc49b6ca0a5a30df72aa88dde6b3e41b3",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "crossing-byte",
            "insert-zero",
            "h6-CurrencyKeys-crossing-byte-insert-zero.accdb",
            "b3ae263ab54664e042644436053d0e04b4757394ad51a800b62b95e12382f92a",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "crossing-byte",
            "delete-all",
            "h6-CurrencyKeys-crossing-byte-delete-all.accdb",
            "9c09a7ec791531c8f610ff27ea3b9d405bbed992e28ad1fa0bb02724bb73a0e4",
        ),
    ] {
        let raw = fs::read(dir.join(filename)).unwrap();
        assert_eq!(digest(&raw), hash);
        let original = &originals[usize::from(header - 5)];
        let definition = match (header, table) {
            (5, "Int64Keys") => 89,
            (5, _) => 96,
            (6, "Int64Keys") => 106,
            (6, _) => 113,
            _ => unreachable!(),
        };
        let data = definition + 5;
        let roots = [definition + 2, definition + 3, definition + 4];
        let before = literal_before(original, definition, data, roots, family);
        let expected = literal_after(&before, definition, data, roots, family, recipe);
        assert_eq!(
            raw, expected,
            "complete original-only byte complement {table}/{family}/{recipe}"
        );
        read_view(&raw, header, table, family, recipe);
    }
}
