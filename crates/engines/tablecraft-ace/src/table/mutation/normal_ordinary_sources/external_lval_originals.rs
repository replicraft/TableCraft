//! Existing SDK originals only; no AFTER model, constructor or mutation calls.
use crate::{
    IndexCheckStatus, LongValueCheckStatus, NativeImage, check_table, check_table_constraints,
    check_table_long_values, inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(193);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn binary(length: usize, salt: u8) -> Vec<u8> {
    (0..length)
        .map(|position| (position as u8).wrapping_mul(73).wrapping_add(salt))
        .collect()
}

fn rows() -> Vec<Vec<Value>> {
    (1..=6)
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
        .collect::<Vec<_>>()
}

fn check_view(raw: &[u8], definition: u32, compressed: bool) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let expected = rows();
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
#[ignore = "requires the four unchanged SDK-created external LVAL originals"]
fn external_lval_sdk_originals_have_full_current_ordinary_source() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_EXTERNAL_LVAL_ORIGINALS_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut views = 0;
    for (name, definition, compressed, bytes, sha) in [
        (
            "V2016-raw",
            89,
            false,
            446464,
            "9a6b7e0e5b0984e51e1af3170ff92e641b48809db2f74c17ab634f80993c0359",
        ),
        (
            "V2016-compressed",
            89,
            true,
            446464,
            "1f5c45d48680ef13c474ebe9b6a6a3e4a745ac7dcdca94c5f3b2b96d82d11c04",
        ),
        (
            "V2019-raw",
            106,
            false,
            520192,
            "986ddd0db110ef562ead874c3c27dbba514a6d2154cb49bf0836d17f3966f125",
        ),
        (
            "V2019-compressed",
            106,
            true,
            520192,
            "e8c39e4b9e52f4204e5e58338d2180efaba980e193c4be078fc84aaf771454de",
        ),
    ] {
        let before = fs::read(directory.join(format!("{name}-source.accdb"))).unwrap();
        assert_eq!(before.len(), bytes);
        assert_eq!(digest(&before), sha);
        check_view(&before, definition, compressed);
        views += 1;
    }
    assert_eq!(views, 4);
}
