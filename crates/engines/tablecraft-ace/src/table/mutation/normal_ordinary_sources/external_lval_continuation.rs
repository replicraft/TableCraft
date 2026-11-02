//! Source-only terminal growth and exclusive page retirement; no mutation calls.
use crate::{
    IndexCheckStatus, LongValueCheckStatus, NativeImage, check_table, check_table_constraints,
    check_table_long_values, inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(193);
const RECIPES: &[&str] = &[
    "grow-shared-last-blob",
    "grow-chain-last-memo",
    "joint-terminal-growth",
    "clear-single-blob",
    "clear-single-memo",
    "clear-single-both",
    "empty-single-blob",
    "clear-memo-chain",
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn word(raw: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(raw[at..at + 2].try_into().unwrap())
}

fn binary(length: usize, salt: u8) -> Vec<u8> {
    (0..length)
        .map(|position| (position as u8).wrapping_mul(73).wrapping_add(salt))
        .collect()
}

fn rows(recipe: &str) -> Vec<Vec<Value>> {
    let mut rows = (1..=6)
        .map(|identity| {
            let group = match identity {
                1 | 5 => Value::Null,
                2 | 3 => Value::Int32(7),
                4 => Value::Int32(8),
                6 => Value::Int32(9),
                _ => unreachable!(),
            };
            let blob = match identity {
                1 => Value::Null,
                2 => Value::Binary(Vec::new()),
                3 | 6 => Value::Binary(binary(65, identity as u8)),
                4 => Value::Binary(binary(4076, 4)),
                5 => Value::Binary(binary(4077, 5)),
                _ => unreachable!(),
            };
            let memo = match identity {
                1 => Value::Null,
                2 => Value::Text(String::new()),
                3 => Value::Text("Ā".repeat(33)),
                4 => Value::Text("Ā".repeat(2038)),
                5 => Value::Text("Ā".repeat(2039)),
                6 => Value::Text("ABC".into()),
                _ => unreachable!(),
            };
            vec![Value::Int32(identity), group, blob, memo]
        })
        .collect::<Vec<_>>();
    match recipe {
        "original" => {}
        "grow-shared-last-blob" => rows[5][2] = Value::Binary(binary(66, 26)),
        "grow-chain-last-memo" => rows[4][3] = Value::Text("β".repeat(2040)),
        "joint-terminal-growth" => {
            rows[5][2] = Value::Binary(binary(66, 26));
            rows[4][3] = Value::Text("β".repeat(2040));
        }
        "clear-single-blob" => rows[2][2] = Value::Null,
        "clear-single-memo" => rows[2][3] = Value::Null,
        "clear-single-both" => {
            rows[2][2] = Value::Null;
            rows[2][3] = Value::Null;
        }
        "empty-single-blob" => rows[2][2] = Value::Binary(Vec::new()),
        "clear-memo-chain" => rows[4][3] = Value::Null,
        _ => panic!("only frozen continuation recipes"),
    }
    rows
}

fn retire(before: &[u8], after: &mut [u8], definition: usize, column: usize, page: usize) {
    let maps = (definition + 1) * 4096;
    let (owned, hint) = if column == 2 {
        (3751, 3682)
    } else {
        (3613, 3544)
    };
    let bit = 1 << (page % 8);
    let owned_at = maps + owned + 5 + page / 8;
    let hint_at = maps + hint + 5 + page / 8;
    let global_at = 4096 + 4027 + 5 + page / 8;
    assert_ne!(before[owned_at] & bit, 0);
    assert_eq!(before[hint_at] & bit, 0);
    assert_eq!(before[global_at] & bit, 0);
    assert_eq!(word(before, page * 4096 + 12), 1);
    assert_eq!(&before[page * 4096..page * 4096 + 2], &[1, 1]);
    assert_eq!(&before[page * 4096 + 4..page * 4096 + 8], b"LVAL");
    after[owned_at] &= !bit;
    after[hint_at] &= !bit;
    after[global_at] |= bit;
    after[page * 4096..page * 4096 + 4].fill(0);
    assert_eq!(
        &after[page * 4096 + 4..(page + 1) * 4096],
        &before[page * 4096 + 4..(page + 1) * 4096]
    );
}

fn expected(before: &[u8], header: u8, compressed: bool, recipe: &str) -> Vec<u8> {
    let mut after = before.to_vec();
    let shift = if header == 5 { 0 } else { 17 };
    let data = 93 + shift;
    let definition = 89 + shift;
    if matches!(recipe, "grow-shared-last-blob" | "joint-terminal-growth") {
        let base = (100 + shift) * 4096;
        assert_eq!(
            (
                word(before, base + 12),
                word(before, base + 16),
                word(before, base + 2)
            ),
            (2, 4022, 4004)
        );
        after[base + 2..base + 4].copy_from_slice(&4003_u16.to_le_bytes());
        after[base + 16..base + 18].copy_from_slice(&4021_u16.to_le_bytes());
        after[base + 4021..base + 4087].copy_from_slice(&binary(66, 26));
        assert_eq!(
            &after[base + 4087..base + 4096],
            &before[base + 4087..base + 4096]
        );
        let parent = data * 4096 + if compressed { 3857 } else { 3856 };
        assert_eq!(
            &before[parent + 10..parent + 14],
            &0x4000_0041_u32.to_le_bytes()
        );
        after[parent + 10..parent + 14].copy_from_slice(&0x4000_0042_u32.to_le_bytes());
    }
    if matches!(recipe, "grow-chain-last-memo" | "joint-terminal-growth") {
        let payload = "β"
            .repeat(2040)
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let first = (101 + shift) * 4096;
        after[first + 24..first + 4096].copy_from_slice(&payload[..4072]);
        let last = (102 + shift) * 4096;
        assert_eq!(
            (
                word(before, last + 12),
                word(before, last + 14),
                word(before, last + 2)
            ),
            (1, 4086, 4070)
        );
        after[last + 2..last + 4].copy_from_slice(&4068_u16.to_le_bytes());
        after[last + 14..last + 16].copy_from_slice(&4084_u16.to_le_bytes());
        after[last + 4084..last + 4088].fill(0);
        after[last + 4088..last + 4096].copy_from_slice(&payload[4072..]);
        let at = data * 4096 + 3905 + 22;
        assert_eq!(&before[at..at + 4], &4078_u32.to_le_bytes());
        after[at..at + 4].copy_from_slice(&4080_u32.to_le_bytes());
    }
    if matches!(
        recipe,
        "clear-single-blob"
            | "clear-single-memo"
            | "clear-single-both"
            | "empty-single-blob"
            | "clear-memo-chain"
    ) {
        let start = if recipe == "clear-memo-chain" {
            3905
        } else {
            3991
        };
        let at = data * 4096 + start;
        let mut blob = before[at + 10..at + 22].to_vec();
        let mut memo = before[at + 22..at + 34].to_vec();
        let mut mask = before[at + 42];
        if matches!(
            recipe,
            "clear-single-blob" | "clear-single-both" | "empty-single-blob"
        ) {
            if recipe == "empty-single-blob" {
                blob.fill(0);
                blob[..4].copy_from_slice(&0x8000_0000_u32.to_le_bytes());
            } else {
                blob.clear();
                mask &= !4;
            }
            retire(before, &mut after, definition, 2, 95 + shift);
        }
        if matches!(
            recipe,
            "clear-single-memo" | "clear-single-both" | "clear-memo-chain"
        ) {
            memo.clear();
            mask &= !8;
            for page in if recipe == "clear-memo-chain" {
                vec![101 + shift, 102 + shift]
            } else {
                vec![96 + shift]
            } {
                retire(before, &mut after, definition, 3, page);
            }
        }
        let end = at + 10 + blob.len() + memo.len();
        after[at + 10..at + 10 + blob.len()].copy_from_slice(&blob);
        after[at + 10 + blob.len()..end].copy_from_slice(&memo);
        for (offset, value) in [
            (34, 10 + blob.len() + memo.len()),
            (36, 10 + blob.len()),
            (38, 10),
            (40, 2),
        ] {
            after[at + offset..at + offset + 2].copy_from_slice(&(value as u16).to_le_bytes());
        }
        after[at + 42] = mask;
        assert_eq!(&after[end..at + 34], &before[end..at + 34]);
    }
    for page in [103 + shift, 104 + shift] {
        assert_eq!(
            &after[page * 4096..(page + 1) * 4096],
            &before[page * 4096..(page + 1) * 4096]
        );
    }
    after
}

fn check_view(raw: &[u8], definition: u32, compressed: bool, recipe: &str) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let expected = rows(recipe);
    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.schema().definition_page(), definition);
    assert_eq!(table.rows(), expected);
    assert_eq!(table.schema().columns().len(), 4);
    assert_eq!(table.schema().columns()[2].native_type(), 11);
    assert_eq!(table.schema().columns()[3].native_type(), 12);
    assert_eq!(table.schema().columns()[2].flags(), 2);
    assert_eq!(table.schema().columns()[3].flags(), 2);
    assert_eq!(
        table.schema().columns()[3].extended_flags(),
        u8::from(compressed)
    );
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 6);
    assert_eq!(checked.indexes().len(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    for (name, order, history) in [
        ("PrimaryID", [0, 1, 2, 3, 4, 5], 7),
        ("PrimaryAlias", [0, 1, 2, 3, 4, 5], 7),
        ("GroupOrder", [0, 4, 1, 2, 3, 5], 5),
        ("GroupAlias", [0, 4, 1, 2, 3, 5], 5),
    ] {
        let read = read_table_index(
            &mut image.reader(),
            SCOPE,
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let wanted = order.map(|index| expected[index].clone());
        assert_eq!(read.rows(), wanted);
        assert_eq!(
            inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                "Items",
                name,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .value(),
            history
        );
    }
    assert_eq!(
        check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap()
        .rows(),
        6
    );
    let storage =
        check_table_long_values(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(storage.rows(), 6);
    assert_eq!(storage.columns().len(), 2);
    for (position, column) in storage.columns().iter().enumerate() {
        assert_eq!(column.column_number(), position + 2);
        assert_eq!(column.status(), LongValueCheckStatus::Checked);
        let retired = if position == 0 {
            usize::from(matches!(
                recipe,
                "clear-single-blob" | "clear-single-both" | "empty-single-blob"
            ))
        } else if recipe == "clear-memo-chain" {
            2
        } else {
            usize::from(matches!(recipe, "clear-single-memo" | "clear-single-both"))
        };
        assert_eq!(column.owned_pages(), 5 - retired);
        assert_eq!(column.candidate_pages(), 1);
        assert_eq!(
            column.owned_rows(),
            (if position == 0 { 6 } else { 5 }) - retired
        );
        assert_eq!(
            column.distinct_chunks(),
            (if position == 0 { 5 } else { 4 }) - retired
        );
        assert_eq!(column.orphan_rows(), 1);
        assert_eq!(column.shared_references(), 0);
        assert_eq!(column.shared_owned_pages(), 0);
    }
    super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    image
        .check_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires independently constructed external continuation sources"]
fn external_lval_growth_and_page_retirement_sources_have_full_ordinary_source() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_EXTERNAL_LVAL_CONTINUATION_SOURCE_DIR").unwrap(),
    );
    assert!(directory.is_absolute());
    let mut contexts = 0;
    for (profile, header, definition, compressed, bytes, sha) in [
        (
            "V2016-raw",
            5,
            89,
            false,
            446464,
            "9a6b7e0e5b0984e51e1af3170ff92e641b48809db2f74c17ab634f80993c0359",
        ),
        (
            "V2016-compressed",
            5,
            89,
            true,
            446464,
            "1f5c45d48680ef13c474ebe9b6a6a3e4a745ac7dcdca94c5f3b2b96d82d11c04",
        ),
        (
            "V2019-raw",
            6,
            106,
            false,
            520192,
            "986ddd0db110ef562ead874c3c27dbba514a6d2154cb49bf0836d17f3966f125",
        ),
        (
            "V2019-compressed",
            6,
            106,
            true,
            520192,
            "e8c39e4b9e52f4204e5e58338d2180efaba980e193c4be078fc84aaf771454de",
        ),
    ] {
        let before = fs::read(directory.join(format!("{profile}-original.accdb"))).unwrap();
        assert_eq!(before.len(), bytes);
        assert_eq!(digest(&before), sha);
        check_view(&before, definition, compressed, "original");
        contexts += 1;
        for recipe in RECIPES {
            let after = fs::read(directory.join(format!("{profile}-{recipe}.accdb"))).unwrap();
            assert_eq!(after, expected(&before, header, compressed, recipe));
            check_view(&after, definition, compressed, recipe);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 36);
}
