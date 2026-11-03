//! Independent original/request replay and read qualification; no writer calls.
use crate::{
    IndexCheckStatus, NativeImage, check_table, check_table_constraints,
    inspect_table_index_counter, read_table, read_table_index, table::mutation::normal_ordinary,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(252);
const TABLES: [&str; 2] = ["Int64Companions", "CurrencyCompanions"];
const ALIASES: [&str; 6] = [
    "PrimaryID",
    "PrimaryAlias",
    "Ascending",
    "AscendingAlias",
    "Descending",
    "DescendingAlias",
];
const PINS: &[(u8, &str, &str, &str, &str)] = &[
    (
        5,
        "",
        "before",
        "h5-before.accdb",
        "fe40ed5d67d0b82c31063b803717eaace70f36724c01951513215382c91e7ef3",
    ),
    (
        6,
        "",
        "before",
        "h6-before.accdb",
        "11d395698885c7a8ef4ea01149118eedba84cdcb00030a33c4923b18abc6de21",
    ),
    (
        5,
        "Int64Companions",
        "minimum-to-zero",
        "h5-Int64Companions-minimum-to-zero.accdb",
        "af6215bc3bf021c1f5caa27e26b75eafa7920cf4f1d892b7ae296c6f02d52f5e",
    ),
    (
        5,
        "Int64Companions",
        "maximum-to-null",
        "h5-Int64Companions-maximum-to-null.accdb",
        "1fe14ab9650c9fa6aac9f751d6e3d4b85c8f3fe46e78627d4e9ac4afd48b8967",
    ),
    (
        5,
        "Int64Companions",
        "null-to-minimum",
        "h5-Int64Companions-null-to-minimum.accdb",
        "40cc2e028f63ac62e7352d9fa679a588fa54c0dbf8a939ad07204a3ede55e865",
    ),
    (
        5,
        "Int64Companions",
        "atomic-extreme-swap",
        "h5-Int64Companions-atomic-extreme-swap.accdb",
        "094e2749257df1b14d70fbae9005afbfc81054003d515481b5e16cf9e5577302",
    ),
    (
        5,
        "CurrencyCompanions",
        "minimum-to-zero",
        "h5-CurrencyCompanions-minimum-to-zero.accdb",
        "1276987d73db5d2d3dd76fabe27e943e351b7381f250bcab80a0ece2cda4e603",
    ),
    (
        5,
        "CurrencyCompanions",
        "maximum-to-null",
        "h5-CurrencyCompanions-maximum-to-null.accdb",
        "8c3a859f0e02cd22abaee73e61c8af45497ba66228b2437fa9ae1d819249bec2",
    ),
    (
        5,
        "CurrencyCompanions",
        "null-to-minimum",
        "h5-CurrencyCompanions-null-to-minimum.accdb",
        "8ac5c2bbb4f03d75b38949e10f5196b27a9437a5828bf52b3a1fb8260dcb0350",
    ),
    (
        5,
        "CurrencyCompanions",
        "atomic-extreme-swap",
        "h5-CurrencyCompanions-atomic-extreme-swap.accdb",
        "074ff489d398c9620e718d767f04492f1fba2b817ffbb99200ae8715a282966b",
    ),
    (
        6,
        "Int64Companions",
        "minimum-to-zero",
        "h6-Int64Companions-minimum-to-zero.accdb",
        "abeb712d57c1d5dc9730c564604bccbbcedc64a4db01ae126fa176d098737415",
    ),
    (
        6,
        "Int64Companions",
        "maximum-to-null",
        "h6-Int64Companions-maximum-to-null.accdb",
        "75550d4bac6b00c6235ea6147e62b1fd381fc2e8b5396425cc0741f8384113e7",
    ),
    (
        6,
        "Int64Companions",
        "null-to-minimum",
        "h6-Int64Companions-null-to-minimum.accdb",
        "3c97bffd5bc00f580147d5e4139b5449994880fd9b2c66e5399996e8129a7fef",
    ),
    (
        6,
        "Int64Companions",
        "atomic-extreme-swap",
        "h6-Int64Companions-atomic-extreme-swap.accdb",
        "2414c9996976b01dc623a9e00c8792bbb1f82304c3c33c7aaed82889dda70c8d",
    ),
    (
        6,
        "CurrencyCompanions",
        "minimum-to-zero",
        "h6-CurrencyCompanions-minimum-to-zero.accdb",
        "3fd69942150f4bbcc61ad3ba3400373147f7549e45b6832fbc9e00df2532d3aa",
    ),
    (
        6,
        "CurrencyCompanions",
        "maximum-to-null",
        "h6-CurrencyCompanions-maximum-to-null.accdb",
        "d2fb55a55c3ed6f99348a8c69ad4dbacdcd3040b061bf46aef3c9b1049daffcf",
    ),
    (
        6,
        "CurrencyCompanions",
        "null-to-minimum",
        "h6-CurrencyCompanions-null-to-minimum.accdb",
        "daeabc7f57ca60a7185e51c1c69ba3e2d19228b9b5becfa569a7a8900e7c7528",
    ),
    (
        6,
        "CurrencyCompanions",
        "atomic-extreme-swap",
        "h6-CurrencyCompanions-atomic-extreme-swap.accdb",
        "4d5ab86713209d25655d623befdfb8c5f7b80022763df0163f7538a4e802ff2b",
    ),
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

fn geometry(header: u8, table: &str) -> (usize, usize, [usize; 3]) {
    match (header, table) {
        (5, "Int64Companions") => (89, 94, [91, 92, 93]),
        (5, "CurrencyCompanions") => (96, 101, [98, 99, 100]),
        (6, "Int64Companions") => (106, 111, [108, 109, 110]),
        (6, "CurrencyCompanions") => (113, 119, [115, 116, 117]),
        _ => panic!("outside qualified original contexts"),
    }
}

fn final_keys(recipe: &str) -> [Option<i64>; 3] {
    match recipe {
        "before" => [Some(i64::MIN), None, Some(i64::MAX)],
        "minimum-to-zero" => [Some(0), None, Some(i64::MAX)],
        "maximum-to-null" => [Some(i64::MIN), None, None],
        "null-to-minimum" => [Some(i64::MIN), Some(i64::MIN), Some(i64::MAX)],
        "atomic-extreme-swap" => [Some(i64::MAX), None, Some(i64::MIN)],
        _ => panic!("outside original request laws"),
    }
}

fn component(value: Option<i64>, descending: bool) -> Vec<u8> {
    let Some(value) = value else {
        return vec![if descending { 255 } else { 0 }];
    };
    let biased = ((value as u64) ^ 0x8000_0000_0000_0000).to_be_bytes();
    let mut wire = vec![if descending { 128 } else { 127 }];
    wire.extend(
        biased
            .into_iter()
            .map(|byte| if descending { !byte } else { byte }),
    );
    wire
}

fn replay(before: &[u8], header: u8, target: &str, recipe: &str) -> Vec<u8> {
    if target.is_empty() {
        assert_eq!(recipe, "before");
        return before.to_vec();
    }
    let (_, data, roots) = geometry(header, target);
    let original = final_keys("before");
    let final_values = final_keys(recipe);
    let mut expected = before.to_vec();
    for ordinal in 0..3 {
        if original[ordinal] == final_values[ordinal] {
            continue;
        }
        let (start, end) = [(4057, 4096), (4018, 4057), (3983, 4018)][ordinal];
        let mask = data * 4096 + end - 1;
        if let Some(value) = final_values[ordinal] {
            expected[data * 4096 + start + 6..data * 4096 + start + 14]
                .copy_from_slice(&value.to_le_bytes());
            expected[mask] |= 2;
        } else {
            expected[mask] &= !2;
        }
    }
    for (physical, root) in roots.iter().enumerate().skip(1) {
        let mut entries = Vec::new();
        for (ordinal, value) in final_values.into_iter().enumerate() {
            let mut entry = component(value, physical == 2);
            entry.extend((((data as u32) << 8) | ordinal as u32).to_be_bytes());
            entries.push(entry);
        }
        entries.sort();
        let mut active = Vec::new();
        let mut bitmap = [0_u8; 453];
        for entry in entries {
            active.extend(entry);
            let end = active.len();
            bitmap[end / 8] |= 1 << (end % 8);
        }
        let base = *root * 4096;
        assert_eq!(&before[base + 20..base + 22], &[0, 0]);
        expected[base + 2..base + 4]
            .copy_from_slice(&(3616_u16 - active.len() as u16).to_le_bytes());
        expected[base + 27..base + 480].copy_from_slice(&bitmap);
        expected[base + 480..base + 480 + active.len()].copy_from_slice(&active);
    }
    expected
}

fn rows(table: &str, recipe: &str) -> Vec<Vec<Value>> {
    let values = final_keys(recipe);
    let mut result = Vec::new();
    for (ordinal, (label, binary)) in [
        ("first", Some(vec![0, 255])),
        ("middle", Some(Vec::new())),
        ("tail", None),
    ]
    .into_iter()
    .enumerate()
    {
        let key = values[ordinal]
            .map(|units| {
                if table == "Int64Companions" {
                    Value::Int64(units)
                } else {
                    Value::Currency(Currency::from_scaled(units))
                }
            })
            .unwrap_or(Value::Null);
        result.push(vec![
            Value::Int32(ordinal as i32 + 1),
            key,
            Value::Int32(17 * (ordinal as i32 + 1)),
            Value::Boolean(ordinal != 1),
            Value::Text(label.into()),
            binary.map(Value::Binary).unwrap_or(Value::Null),
        ]);
    }
    result
}

#[test]
#[ignore = "requires independently constructed AFTER sources; no native mutation calls"]
fn wider_numeric_sibling_after_sources_match_literal_requests_and_complete_canonical_reads() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_WIDE_SIBLING_AFTER_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    assert_eq!(
        digest(&fs::read(directory.join("manifest.json")).unwrap()),
        "5cb0f5c8e795f3efb29abaac04eb40366b3f16e225a7fae40f71dd5f9e9496ed"
    );
    let images: Vec<_> = PINS
        .iter()
        .map(|&(header, table, recipe, file, expected)| {
            let raw = fs::read(directory.join(file)).unwrap();
            assert_eq!(digest(&raw), expected);
            (header, table, recipe, raw)
        })
        .collect();
    assert_eq!(images.len(), 18);
    let mut contexts = 0;
    for (header, target, recipe, raw) in &images {
        let before = &images
            .iter()
            .find(|item| item.0 == *header && item.1.is_empty())
            .unwrap()
            .3;
        assert_eq!(*raw, replay(before, *header, target, recipe));
        for table in TABLES {
            let actual_recipe = if *target == table { *recipe } else { "before" };
            let expected = rows(table, actual_recipe);
            let (definition, data, roots) = geometry(*header, table);
            let image =
                NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let snapshot =
                read_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
            assert_eq!(snapshot.rows(), expected);
            assert_eq!(snapshot.schema().definition_page() as usize, definition);
            assert_eq!(snapshot.schema().columns().len(), 6);
            let checked =
                check_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
            assert_eq!(checked.rows(), 3);
            assert_eq!(checked.indexes().len(), 6);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            let constraints =
                check_table_constraints(&mut image.reader(), SCOPE, table, &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert!(constraints.relationships().is_empty());
            for (position, alias) in ALIASES.into_iter().enumerate() {
                let physical = position / 2;
                let mut order: Vec<_> = (0..3).collect();
                if physical != 0 {
                    let values = final_keys(actual_recipe);
                    order.sort_by_key(|ordinal| {
                        let value = values[*ordinal];
                        if physical == 1 {
                            (
                                u8::from(value.is_some()),
                                value.map(i128::from).unwrap_or(0),
                                *ordinal,
                            )
                        } else {
                            (
                                u8::from(value.is_none()),
                                value.map(|n| -i128::from(n)).unwrap_or(0),
                                *ordinal,
                            )
                        }
                    });
                }
                let observed = read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    table,
                    alias,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let ordered: Vec<_> = order
                    .into_iter()
                    .map(|ordinal| expected[ordinal].clone())
                    .collect();
                assert_eq!(observed.rows(), ordered);
                let counter = inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    table,
                    alias,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(counter.value(), 3);
                let base = roots[physical] * 4096;
                assert_eq!(&raw[base + 20..base + 22], &[0, 0]);
            }
            let source = normal_ordinary::capture(&image, table, &mut budget(), &|| false).unwrap();
            assert_eq!(source.table.rows(), expected);
            assert_eq!(source.records.len(), 3);
            for (ordinal, record) in source.records.iter().enumerate() {
                assert_eq!(record.reference, ((data as u32) << 8) | ordinal as u32);
            }
            contexts += 1;
        }
    }
    assert_eq!(contexts, 36);
}
