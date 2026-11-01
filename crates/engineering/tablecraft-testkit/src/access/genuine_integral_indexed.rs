//! Original/literal sources shared by real native and host consumers; std/core only.
use std::{fs, path::PathBuf};
use tablecraft_core::values::{Currency, Value};

pub struct Case {
    pub header: u8,
    pub table: &'static str,
    pub operation: &'static str,
    pub recipe: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 3],
}

pub fn value(table: &str, id: i32, key: Option<i64>, marker: i32) -> Vec<Value> {
    let key = key
        .map(|key| {
            if table == "CurrencyKeys" {
                Value::Currency(Currency::from_scaled(key))
            } else {
                Value::Int64(key)
            }
        })
        .unwrap_or(Value::Null);
    vec![Value::Int32(id), key, Value::Int32(marker)]
}

pub fn insert_values(table: &str, recipe: &str) -> Vec<Vec<Value>> {
    match recipe {
        "null-family" => vec![value(table, 4, None, 68)],
        "existing-family" => vec![value(table, 4, Some(i64::MIN), 68)],
        "new-family" => vec![value(table, 4, Some(0), 68)],
        "batch-families" => vec![
            value(table, 4, None, 68),
            value(table, 5, Some(i64::MIN), 85),
            value(table, 6, Some(0), 102),
        ],
        _ => panic!("outside source-qualified requests"),
    }
}

pub fn ordinals(recipe: &str, reverse: bool) -> Vec<usize> {
    let mut rows = match recipe {
        "minimum" => vec![0],
        "null" => vec![1],
        "exterior" => vec![0, 2],
        "all" => vec![0, 1, 2],
        _ => panic!("outside source-qualified removals"),
    };
    if reverse {
        rows.reverse();
    }
    rows
}

pub fn record(slot: usize) -> Vec<u8> {
    let mut raw = Vec::from([3, 0]);
    raw.extend_from_slice(&((slot + 1) as i32).to_le_bytes());
    raw.extend_from_slice(&(if slot == 2 { i64::MAX } else { i64::MIN }).to_le_bytes());
    raw.extend_from_slice(&(((slot + 1) * 17) as i32).to_le_bytes());
    raw.push(if slot == 1 { 5 } else { 7 });
    assert_eq!(raw.len(), 19);
    raw
}

pub fn rows(case: &Case) -> Vec<Vec<Value>> {
    let original = [
        value(case.table, 1, Some(i64::MIN), 17),
        value(case.table, 2, None, 34),
        value(case.table, 3, Some(i64::MAX), 51),
    ];
    if case.operation == "insert" {
        let mut result = original.to_vec();
        result.extend(insert_values(case.table, case.recipe));
        result
    } else {
        let selected = ordinals(case.recipe, false);
        original
            .into_iter()
            .enumerate()
            .filter_map(|(slot, row)| (!selected.contains(&slot)).then_some(row))
            .collect()
    }
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_INDEXED_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut originals = Vec::new();
    let mut result = Vec::new();
    for (header, table, operation, recipe, name, length, hash) in [
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
        verify(&raw, hash);
        if operation == "original" {
            originals.push(raw);
            continue;
        }
        let definition = match (header, table) {
            (5, "Int64Keys") => 89,
            (5, _) => 96,
            (6, "Int64Keys") => 106,
            (6, _) => 113,
            _ => unreachable!(),
        };
        result.push(Case {
            header,
            table,
            operation,
            recipe,
            before: originals[usize::from(header - 5)].clone(),
            after: raw,
            definition,
            data: definition + 5,
            roots: [definition + 2, definition + 3, definition + 4],
        });
    }
    assert_eq!(result.len(), 32);
    result
}
