//! Original/request-derived DATA and key overlays, read-only native qualification.
use crate::{
    IndexCheckStatus, NativeImage, check_table, check_table_constraints,
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

fn rows(operation: &str, recipe: &str) -> Vec<Row> {
    let mut rows = vec![
        (0, 1, Some(i64::MIN), 17),
        (1, 2, None, 34),
        (2, 3, Some(i64::MAX), 51),
    ];
    match operation {
        "original" => assert_eq!(recipe, "original"),
        "insert" => match recipe {
            "null-family" => rows.push((3, 4, None, 68)),
            "existing-family" => rows.push((3, 4, Some(i64::MIN), 68)),
            "new-family" => rows.push((3, 4, Some(0), 68)),
            "batch-families" => rows.extend([
                (3, 4, None, 68),
                (4, 5, Some(i64::MIN), 85),
                (5, 6, Some(0), 102),
            ]),
            _ => panic!("unknown literal insertion recipe"),
        },
        "delete" => rows.retain(|row| match recipe {
            "minimum" => row.0 != 0,
            "null" => row.0 != 1,
            "exterior" => row.0 == 1,
            "all" => false,
            _ => panic!("unknown literal deletion recipe"),
        }),
        _ => panic!("unknown source operation"),
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

fn history(operation: &str, recipe: &str, physical: usize) -> u32 {
    if operation != "insert" {
        return 3;
    }
    if physical == 0 {
        return if recipe == "batch-families" { 6 } else { 4 };
    }
    if matches!(recipe, "new-family" | "batch-families") {
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
                for byte in payload.iter_mut() {
                    *byte = !*byte;
                }
            }
            result.extend_from_slice(&payload);
        }
    }
    result.extend_from_slice(&((data << 8) | row.0 as u32).to_be_bytes());
    result
}

fn literal(
    before: &[u8],
    definition: u32,
    data: u32,
    roots: [u32; 3],
    operation: &str,
    recipe: &str,
) -> Vec<u8> {
    let mut after = before.to_vec();
    let states = rows(operation, recipe);
    let td = definition as usize * 4096;
    let page = data as usize * 4096;
    after[td + 16..td + 20].copy_from_slice(&(states.len() as u32).to_le_bytes());
    if operation == "insert" {
        for row in states.iter().skip(3) {
            let start = 4039 - 19 * (row.0 - 2);
            let mut record = Vec::from([3, 0]);
            record.extend_from_slice(&row.1.to_le_bytes());
            record.extend_from_slice(&row.2.unwrap_or(0).to_le_bytes());
            record.extend_from_slice(&row.3.to_le_bytes());
            record.push(if row.2.is_some() { 7 } else { 5 });
            assert_eq!(record.len(), 19);
            after[page + start..page + start + 19].copy_from_slice(&record);
            after[page + 14 + row.0 * 2..page + 16 + row.0 * 2]
                .copy_from_slice(&(start as u16).to_le_bytes());
        }
        let end = 4039 - 19 * (states.len() - 3);
        let free = end - 14 - 2 * states.len();
        after[page + 2..page + 4].copy_from_slice(&(free as u16).to_le_bytes());
        after[page + 12..page + 14].copy_from_slice(&(states.len() as u16).to_le_bytes());
    } else if operation == "delete" {
        for slot in 0..3 {
            if !states.iter().any(|row| row.0 == slot) {
                after[page + 14 + slot * 2..page + 16 + slot * 2]
                    .copy_from_slice(&((4077_u16 - slot as u16 * 19) | 0xc000).to_le_bytes());
            }
        }
    }
    for (physical, root) in roots.iter().enumerate() {
        let base = *root as usize * 4096;
        let mut ordered = states.clone();
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
        after[td + 67 + 12 * physical..td + 71 + 12 * physical]
            .copy_from_slice(&history(operation, recipe, physical).to_le_bytes());
    }
    after
}

fn read_view(raw: &[u8], header: u8, selected: &str, operation: &str, recipe: &str) {
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
        let roots = [definition + 2, definition + 3, definition + 4];
        let active = if selected == table {
            rows(operation, recipe)
        } else {
            rows("original", "original")
        };
        let expected = active
            .iter()
            .map(|row| values(table, *row))
            .collect::<Vec<_>>();
        let snapshot =
            read_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
        assert_eq!(snapshot.rows(), expected);
        assert_eq!(snapshot.schema().definition_page(), definition);
        let fields = snapshot.schema().columns();
        assert_eq!(fields.len(), 3);
        assert_eq!(
            (
                fields[1].name(),
                fields[1].native_type(),
                fields[1].flags(),
                fields[1].storage_length()
            ),
            (
                "KeyValue",
                if table == "CurrencyKeys" { 5 } else { 19 },
                3,
                8
            )
        );
        let constraints =
            check_table_constraints(&mut image.reader(), SCOPE, table, &mut budget(), &|| false)
                .unwrap();
        assert_eq!(constraints.rows(), active.len());
        assert!(constraints.relationships().is_empty());
        let checked =
            check_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
        assert_eq!(checked.rows(), active.len());
        assert_eq!(checked.indexes().len(), 6);
        for (position, check) in checked.indexes().iter().enumerate() {
            let physical = position / 2;
            assert_eq!(check.status(), IndexCheckStatus::Checked);
            let index = check.index();
            assert_eq!(index.name(), ALIASES[position]);
            assert_eq!(index.data_number(), physical as u32);
            assert_eq!(index.root_page(), roots[physical]);
            assert_eq!(
                index.usage_map(),
                ((definition + 1) << 8) | (2 + physical as u32)
            );
            assert_eq!(index.flags(), if physical == 0 { 137 } else { 128 });
            assert_eq!(index.columns().len(), 1);
            assert_eq!(
                index.columns()[0].number(),
                if physical == 0 { 0 } else { 1 }
            );
            assert_eq!(index.columns()[0].descending(), physical == 2);
            let counter = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                table,
                ALIASES[position],
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                counter.value(),
                if selected == table {
                    history(operation, recipe, physical)
                } else {
                    3
                }
            );
            let mut ordered = active.clone();
            order(&mut ordered, physical);
            let ordered = ordered
                .into_iter()
                .map(|row| values(table, row))
                .collect::<Vec<_>>();
            let read = read_table_index(
                &mut image.reader(),
                SCOPE,
                table,
                ALIASES[position],
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(read.rows(), ordered);
        }
        super::super::normal_ordinary::capture(&image, table, &mut budget(), &|| false).unwrap();
        image
            .check_native_ordinary_fixed_table(table, &mut budget(), &|| false)
            .unwrap();
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires34 independently constructed and SDK-qualified integral source views"]
fn integral_currency_literal_insert_delete_views_have_exact_complete_membership_and_source() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_INDEXED_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    assert_eq!(
        digest(&fs::read(directory.join("manifest.json")).unwrap()),
        "359cec123016c1b9bb4d0d3a5e5891c6b3af6ffacaabb21860db32962dea721d"
    );
    let mut originals = Vec::new();
    for (header, selected, operation, recipe, name, length, hash) in [
        (
            5_u8,
            "",
            "original",
            "original",
            "h5-integral-original.accdb",
            421888_usize,
            "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
        ),
        (
            5_u8,
            "Int64Keys",
            "insert",
            "null-family",
            "h5-integral-Int64Keys-insert-null-family.accdb",
            421888_usize,
            "ca5250b7b9044d0dd55b18ba29489081f0414205f5321853f8b6832fa934c038",
        ),
        (
            5_u8,
            "Int64Keys",
            "insert",
            "existing-family",
            "h5-integral-Int64Keys-insert-existing-family.accdb",
            421888_usize,
            "85b34e57c23e4f59263d4bb8527a09594f3646455089a277429fdf7899518be1",
        ),
        (
            5_u8,
            "Int64Keys",
            "insert",
            "new-family",
            "h5-integral-Int64Keys-insert-new-family.accdb",
            421888_usize,
            "fce88a646d6bfb7298a95c651bd5ce3bf73f9cfab2dd60203659fd11c0ba4922",
        ),
        (
            5_u8,
            "Int64Keys",
            "insert",
            "batch-families",
            "h5-integral-Int64Keys-insert-batch-families.accdb",
            421888_usize,
            "a7c92e3b56cae4768f9c3f122345ae05dbe9a910d4970ceb727755d1a01c3a6b",
        ),
        (
            5_u8,
            "Int64Keys",
            "delete",
            "minimum",
            "h5-integral-Int64Keys-delete-minimum.accdb",
            421888_usize,
            "ea0bedee15e2fa8ec0b7eba53bd36a7268dc55b1e7b2688211028aca707bb11d",
        ),
        (
            5_u8,
            "Int64Keys",
            "delete",
            "null",
            "h5-integral-Int64Keys-delete-null.accdb",
            421888_usize,
            "aefc39ee21e463b3d0370ca68d8d6d2755bd84857a4e010659c7bda2c8de5f31",
        ),
        (
            5_u8,
            "Int64Keys",
            "delete",
            "exterior",
            "h5-integral-Int64Keys-delete-exterior.accdb",
            421888_usize,
            "96c2b8eeb295d0e7c1fdd6626a82130afd9c7eb82f5652768f7fe903c9226d8a",
        ),
        (
            5_u8,
            "Int64Keys",
            "delete",
            "all",
            "h5-integral-Int64Keys-delete-all.accdb",
            421888_usize,
            "c73a65b461f0d52a73ec47aeba77e40ecf1d2c6686744360d4ca977049690dc8",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "insert",
            "null-family",
            "h5-integral-CurrencyKeys-insert-null-family.accdb",
            421888_usize,
            "a9328a59125e1f9fd0f7a7d9125e566678b15751314e164493fdf23b95aa33ba",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "insert",
            "existing-family",
            "h5-integral-CurrencyKeys-insert-existing-family.accdb",
            421888_usize,
            "e3b9436331ead768c52ae25a3ce8e87c1aa51ed388890206bd001fb3c6d1ee66",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "insert",
            "new-family",
            "h5-integral-CurrencyKeys-insert-new-family.accdb",
            421888_usize,
            "b5bcbc55247302dfa09c19485c8cbc26f869c042723d037a394408f8fae9d354",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "insert",
            "batch-families",
            "h5-integral-CurrencyKeys-insert-batch-families.accdb",
            421888_usize,
            "7725705f829dd80184ac87290776c2a7d025046a145ce5b5e8e61d4e65391058",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "delete",
            "minimum",
            "h5-integral-CurrencyKeys-delete-minimum.accdb",
            421888_usize,
            "abc408acad29efca92fb71f1c15e5e3c2ba66cf72c264ae3ad7431abec34aaee",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "delete",
            "null",
            "h5-integral-CurrencyKeys-delete-null.accdb",
            421888_usize,
            "cf62f124203d0177c721cf5f694db177267e6b7bbf42f0ee155a3efb62d7c204",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "delete",
            "exterior",
            "h5-integral-CurrencyKeys-delete-exterior.accdb",
            421888_usize,
            "2ca777b07727fbb2035a0810aea08525c108988e0e12dd93520bad906331914d",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "delete",
            "all",
            "h5-integral-CurrencyKeys-delete-all.accdb",
            421888_usize,
            "6dbfe1d18755164a42b0b6567d13cae9e2183cd51ff12b65614b98071a83ccf3",
        ),
        (
            6_u8,
            "",
            "original",
            "original",
            "h6-integral-original.accdb",
            491520_usize,
            "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
        ),
        (
            6_u8,
            "Int64Keys",
            "insert",
            "null-family",
            "h6-integral-Int64Keys-insert-null-family.accdb",
            491520_usize,
            "ebe210946ad496ff08dbe76371107ad8e68546d7db430b5db874f6f3f44a9551",
        ),
        (
            6_u8,
            "Int64Keys",
            "insert",
            "existing-family",
            "h6-integral-Int64Keys-insert-existing-family.accdb",
            491520_usize,
            "9d870efc4bf4b81c6a9272af2595fb9d89ad70090812a03c34a38602ce3c5018",
        ),
        (
            6_u8,
            "Int64Keys",
            "insert",
            "new-family",
            "h6-integral-Int64Keys-insert-new-family.accdb",
            491520_usize,
            "56026f65eb9af31b9ac62f7104239073cf809c0167c4e2b30b03cc08cbea2f42",
        ),
        (
            6_u8,
            "Int64Keys",
            "insert",
            "batch-families",
            "h6-integral-Int64Keys-insert-batch-families.accdb",
            491520_usize,
            "f5a979d7bce18ce6f6b997ea27bbe25a08672a257504fe4291bbe24041e125bf",
        ),
        (
            6_u8,
            "Int64Keys",
            "delete",
            "minimum",
            "h6-integral-Int64Keys-delete-minimum.accdb",
            491520_usize,
            "485c2262c09ab87a65402bd810441c4d8ed3e76aaf2e62d5f41bcf151f0d0b1c",
        ),
        (
            6_u8,
            "Int64Keys",
            "delete",
            "null",
            "h6-integral-Int64Keys-delete-null.accdb",
            491520_usize,
            "15e9897f1c56c0ed8c31c80f19443fb865e4e1402841767f584bd3851de1be98",
        ),
        (
            6_u8,
            "Int64Keys",
            "delete",
            "exterior",
            "h6-integral-Int64Keys-delete-exterior.accdb",
            491520_usize,
            "b90a5dcb7d2586b42fa0633e17a57f7e97b02cec29430a5c50d2095d0f574b79",
        ),
        (
            6_u8,
            "Int64Keys",
            "delete",
            "all",
            "h6-integral-Int64Keys-delete-all.accdb",
            491520_usize,
            "97ec82868ed17533051bb0345864b642a44951d45b41fcd9ffffc61437589a0b",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "insert",
            "null-family",
            "h6-integral-CurrencyKeys-insert-null-family.accdb",
            491520_usize,
            "fdb93116f80e0f22be44eed60bc74cbfbe7d5c70aa7471b5c72cd75a7f58094c",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "insert",
            "existing-family",
            "h6-integral-CurrencyKeys-insert-existing-family.accdb",
            491520_usize,
            "bd647f14f8fa00f4db2a80fa1661b6151ebe52ce07a2c02b42e618c288ebbb30",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "insert",
            "new-family",
            "h6-integral-CurrencyKeys-insert-new-family.accdb",
            491520_usize,
            "ce55ae3a1dbc6f52afab2d5c73cac0a5a1db02d6e1e45149222b5e8a3b424b91",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "insert",
            "batch-families",
            "h6-integral-CurrencyKeys-insert-batch-families.accdb",
            491520_usize,
            "30dae27a14457d5003e26933f55e2924cbf31a62483635d4a2e6d7603dc58480",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "delete",
            "minimum",
            "h6-integral-CurrencyKeys-delete-minimum.accdb",
            491520_usize,
            "58270930d4e5fca3671687aaa3989603092a41579c3cefbbf8c34709c4aba552",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "delete",
            "null",
            "h6-integral-CurrencyKeys-delete-null.accdb",
            491520_usize,
            "acf3a0eefbe59e9ccad4dee23e7edc4864843c6b639162565736b4d5e3e35ad3",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "delete",
            "exterior",
            "h6-integral-CurrencyKeys-delete-exterior.accdb",
            491520_usize,
            "28a1bfb0971cb7c614464893a584459cc99d5958c81f6ee506bb205a2073e759",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "delete",
            "all",
            "h6-integral-CurrencyKeys-delete-all.accdb",
            491520_usize,
            "9600a6e1449f28485af22006de9730f9c4ff013d712a0606c38715fc1ac9a35a",
        ),
    ] {
        let raw = fs::read(directory.join(name)).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(digest(&raw), hash);
        if operation == "original" {
            originals.push(raw.clone());
        } else {
            let definition = match (header, selected) {
                (5, "Int64Keys") => 89,
                (5, _) => 96,
                (6, "Int64Keys") => 106,
                (6, _) => 113,
                _ => unreachable!(),
            };
            let before = &originals[usize::from(header - 5)];
            let expected = literal(
                before,
                definition,
                definition + 5,
                [definition + 2, definition + 3, definition + 4],
                operation,
                recipe,
            );
            assert_eq!(
                raw, expected,
                "complete independent raw overlay {selected}/{operation}/{recipe}"
            );
        }
        read_view(&raw, header, selected, operation, recipe);
    }
}
