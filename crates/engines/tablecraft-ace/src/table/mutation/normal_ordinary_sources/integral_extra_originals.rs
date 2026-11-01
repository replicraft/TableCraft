//! Read/constraint/metadata evidence only; no normal mutation or intent service.
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

const SCOPE: CatalogScope = CatalogScope::new(200);
const ALIASES: [&str; 6] = [
    "PrimaryID",
    "PrimaryAlias",
    "Ascending",
    "AscendingAlias",
    "Descending",
    "DescendingAlias",
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

fn numeric(table: &str, value: i64) -> Value {
    if table.starts_with("Currency") {
        Value::Currency(Currency::from_scaled(value))
    } else {
        Value::Int64(value)
    }
}

fn values(table: &str, slot: usize) -> Vec<Value> {
    let key = [i64::MIN, 0, i64::MAX][slot];
    let id = if table.ends_with("WidePrimary") {
        numeric(table, key)
    } else {
        Value::Int32((slot + 1) as i32)
    };
    let value = if table.ends_with("AllNull") || slot == 1 {
        Value::Null
    } else {
        numeric(table, key)
    };
    vec![id, value, Value::Int32(((slot + 1) * 17) as i32)]
}

fn row_order(table: &str, physical: usize) -> Vec<usize> {
    let mut slots = vec![0, 1, 2];
    slots.sort_by(|left, right| {
        let key = |slot: usize| {
            if physical == 0 {
                Some(if table.ends_with("WidePrimary") {
                    [i64::MIN, 0, i64::MAX][slot]
                } else {
                    (slot + 1) as i64
                })
            } else if table.ends_with("AllNull") || slot == 1 {
                None
            } else {
                Some([i64::MIN, 0, i64::MAX][slot])
            }
        };
        let comparison = key(*left).cmp(&key(*right));
        let comparison = if physical == 2 {
            comparison.reverse()
        } else {
            comparison
        };
        if comparison == Ordering::Equal {
            left.cmp(right)
        } else {
            comparison
        }
    });
    slots
}

#[test]
#[ignore = "requires two fresh SDK originals and76 complete inert observations"]
fn extra_integral_originals_separate_native_readability_from_current_source_limits() {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_EXTRA_ORIGINALS_DIR").unwrap());
    assert!(dir.is_absolute());
    for (header, length, hash) in [
        (
            5_u8,
            540672_usize,
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
        ),
        (
            6_u8,
            610304_usize,
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
        ),
    ] {
        let raw = fs::read(dir.join(format!("h{header}-integral-extra-original.accdb"))).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(digest(&raw), hash);
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        for (selected_header, table, definition, data, roots) in [
            (5_u8, "Int64AllNull", 89_u32, 94_u32, [91, 92, 93]),
            (5_u8, "CurrencyAllNull", 96_u32, 101_u32, [98, 99, 100]),
            (5_u8, "Int64WidePrimary", 103_u32, 108_u32, [105, 106, 107]),
            (
                5_u8,
                "CurrencyWidePrimary",
                110_u32,
                115_u32,
                [112, 113, 114],
            ),
            (5_u8, "Int64StableIds", 117_u32, 122_u32, [119, 120, 121]),
            (5_u8, "CurrencyStableIds", 124_u32, 130_u32, [126, 127, 128]),
            (6_u8, "Int64AllNull", 106_u32, 111_u32, [108, 109, 110]),
            (6_u8, "CurrencyAllNull", 113_u32, 118_u32, [115, 116, 117]),
            (6_u8, "Int64WidePrimary", 120_u32, 126_u32, [122, 123, 124]),
            (
                6_u8,
                "CurrencyWidePrimary",
                128_u32,
                133_u32,
                [130, 131, 132],
            ),
            (6_u8, "Int64StableIds", 135_u32, 140_u32, [137, 138, 139]),
            (6_u8, "CurrencyStableIds", 142_u32, 147_u32, [144, 145, 146]),
        ] {
            if selected_header != header {
                continue;
            }
            let snapshot =
                read_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
            assert_eq!(snapshot.schema().definition_page(), definition);
            let expected = (0..3).map(|slot| values(table, slot)).collect::<Vec<_>>();
            assert_eq!(snapshot.rows(), expected);
            let fields = snapshot.schema().columns();
            assert_eq!(fields.len(), 3);
            let wide = table.ends_with("WidePrimary");
            let kind = if table.starts_with("Currency") { 5 } else { 19 };
            let widths = [if wide { 8 } else { 4 }, 8, 4];
            for (position, field) in fields.iter().enumerate() {
                assert_eq!(field.number(), position);
                assert_eq!(field.storage_length(), widths[position]);
                assert_eq!(
                    field.native_type(),
                    if position == 1 || position == 0 && wide {
                        kind
                    } else {
                        4
                    }
                );
                let at = definition as usize * 4096 + 99 + 25 * position + 9;
                let stored_id = u16::from_le_bytes(raw[at..at + 2].try_into().unwrap());
                assert_eq!(
                    stored_id,
                    if table.ends_with("StableIds") {
                        [0, 3, 4][position]
                    } else {
                        position as u16
                    }
                );
            }
            let data_page = data as usize * 4096;
            for (slot, _) in expected.iter().enumerate() {
                let at = data_page + 14 + slot * 2;
                let start = usize::from(u16::from_le_bytes(raw[at..at + 2].try_into().unwrap()));
                let end = if slot == 0 {
                    4096
                } else {
                    usize::from(u16::from_le_bytes(raw[at - 2..at].try_into().unwrap()))
                };
                assert_eq!(end - start, if wide { 23 } else { 19 });
                assert_eq!(
                    raw[data_page + end - 1],
                    if table.ends_with("AllNull") || slot == 1 {
                        5
                    } else {
                        7
                    }
                );
            }
            let constraints =
                check_table_constraints(&mut image.reader(), SCOPE, table, &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(constraints.rows(), 3);
            assert!(constraints.relationships().is_empty());
            let checked =
                check_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
            assert_eq!(checked.rows(), 3);
            assert_eq!(checked.indexes().len(), 6);
            for (position, check) in checked.indexes().iter().enumerate() {
                assert_eq!(check.status(), IndexCheckStatus::Checked);
                let index = check.index();
                let physical = position / 2;
                assert_eq!(index.name(), ALIASES[position]);
                assert_eq!(index.root_page(), roots[physical]);
                assert_eq!(index.columns().len(), 1);
                assert_eq!(index.columns()[0].descending(), physical == 2);
                let counter = inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    table,
                    index.name(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    counter.value(),
                    if table.ends_with("AllNull") && physical != 0 {
                        1
                    } else {
                        3
                    }
                );
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
                    row_order(table, physical)
                        .into_iter()
                        .map(|slot| values(table, slot))
                        .collect::<Vec<_>>()
                );
            }
            // Stable IDs are readable historical words, but the current strict
            // selected-TD grammar canonically demands ID==dense position.
            if table.ends_with("StableIds") {
                let error =
                    super::super::normal_ordinary::capture(&image, table, &mut budget(), &|| false)
                        .err()
                        .unwrap();
                assert_eq!(error, EditError::Unsupported(WriteBlocker::ActiveContent));
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
}
