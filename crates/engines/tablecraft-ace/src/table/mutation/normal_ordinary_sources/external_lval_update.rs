//! Source-only live LVAL reuse/shrink reads; no native mutation or encoder calls.
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
    "joint-single65-66",
    "joint-single4076",
    "joint-chain4077-4078",
    "memo-last-tail-shrink",
    "blob-last-slot-shrink",
    "noop-chain",
    "empty-group",
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
        "original" | "noop-chain" | "empty-group" => {}
        "joint-single65-66" => {
            rows[2][2] = Value::Binary(binary(65, 13));
            rows[2][3] = Value::Text("β".repeat(33));
        }
        "joint-single4076" => {
            rows[3][2] = Value::Binary(binary(4076, 14));
            rows[3][3] = Value::Text("β".repeat(2038));
        }
        "joint-chain4077-4078" => {
            rows[4][2] = Value::Binary(binary(4077, 15));
            rows[4][3] = Value::Text("β".repeat(2039));
        }
        "memo-last-tail-shrink" => rows[4][3] = Value::Text("β".repeat(2038)),
        "blob-last-slot-shrink" => rows[5][2] = Value::Binary(binary(63, 16)),
        _ => panic!("only frozen source recipes"),
    }
    rows
}

fn chunk(after: &mut [u8], page: usize, slot: usize, start: usize, skip: usize, payload: &[u8]) {
    let base = page * 4096;
    assert_eq!(&after[base..base + 2], &[1, 1]);
    assert_eq!(&after[base + 4..base + 8], b"LVAL");
    assert_eq!(usize::from(word(after, base + 14 + 2 * slot)), start);
    let end = if slot == 0 {
        4096
    } else {
        usize::from(word(after, base + 12 + 2 * slot))
    };
    assert_eq!(start + skip + payload.len(), end);
    after[base + start + skip..base + end].copy_from_slice(payload);
}

fn expected(before: &[u8], header: u8, compressed: bool, recipe: &str) -> Vec<u8> {
    let mut after = before.to_vec();
    let shift = if header == 5 { 0 } else { 17 };
    let data = 93 + shift;
    let beta = |count: usize| {
        "β".repeat(count)
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    match recipe {
        "noop-chain" | "empty-group" => {}
        "joint-single65-66" => {
            chunk(&mut after, 95 + shift, 0, 4031, 0, &binary(65, 13));
            chunk(&mut after, 96 + shift, 0, 4030, 0, &beta(33));
        }
        "joint-single4076" => {
            chunk(&mut after, 97 + shift, 0, 20, 0, &binary(4076, 14));
            chunk(&mut after, 98 + shift, 0, 20, 0, &beta(2038));
        }
        "joint-chain4077-4078" => {
            let blob = binary(4077, 15);
            let memo = beta(2039);
            chunk(&mut after, 99 + shift, 0, 20, 4, &blob[..4072]);
            chunk(&mut after, 100 + shift, 0, 4087, 4, &blob[4072..]);
            chunk(&mut after, 101 + shift, 0, 20, 4, &memo[..4072]);
            chunk(&mut after, 102 + shift, 0, 4086, 4, &memo[4072..]);
        }
        "memo-last-tail-shrink" => {
            let memo = beta(2038);
            chunk(&mut after, 101 + shift, 0, 20, 4, &memo[..4072]);
            let base = (102 + shift) * 4096;
            assert_eq!(word(&after, base + 12), 1);
            assert_eq!(word(&after, base + 14), 4086);
            assert_eq!(word(&after, base + 2), 4070);
            after[base + 2..base + 4].copy_from_slice(&4072_u16.to_le_bytes());
            after[base + 14..base + 16].copy_from_slice(&4088_u16.to_le_bytes());
            after[base + 4088..base + 4092].copy_from_slice(&[0; 4]);
            after[base + 4092..base + 4096].copy_from_slice(&memo[4072..]);
            let descriptor = data * 4096 + 3905 + 22;
            assert_eq!(&before[descriptor..descriptor + 4], &4078_u32.to_le_bytes());
            after[descriptor..descriptor + 4].copy_from_slice(&4076_u32.to_le_bytes());
        }
        "blob-last-slot-shrink" => {
            let base = (100 + shift) * 4096;
            assert_eq!(word(&after, base + 12), 2);
            assert_eq!(word(&after, base + 16), 4022);
            assert_eq!(word(&after, base + 2), 4004);
            after[base + 2..base + 4].copy_from_slice(&4006_u16.to_le_bytes());
            after[base + 16..base + 18].copy_from_slice(&4024_u16.to_le_bytes());
            after[base + 4024..base + 4087].copy_from_slice(&binary(63, 16));
            let parent = if compressed { 3857 } else { 3856 };
            let descriptor = data * 4096 + parent + 10;
            assert_eq!(
                &before[descriptor..descriptor + 4],
                &0x4000_0041_u32.to_le_bytes()
            );
            after[descriptor..descriptor + 4].copy_from_slice(&0x4000_003f_u32.to_le_bytes());
        }
        _ => panic!("only frozen source recipes"),
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
        assert_eq!(column.owned_pages(), 5);
        assert_eq!(column.owned_rows(), if position == 0 { 6 } else { 5 });
        assert_eq!(column.distinct_chunks(), if position == 0 { 5 } else { 4 });
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
#[ignore = "requires independently constructed external live-chunk reuse sources"]
fn external_lval_reuse_sources_read_full_rows_aliases_orphans_and_ordinary_source() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_EXTERNAL_LVAL_UPDATE_SOURCE_DIR").unwrap());
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
    assert_eq!(contexts, 32);
}
