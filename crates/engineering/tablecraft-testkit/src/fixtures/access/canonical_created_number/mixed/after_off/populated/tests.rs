//! Historical record counts select NULL rather than the newly stored default.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{inspect_table_properties, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

#[test]
fn number_add_retains_historical_records_and_exposes_missing_new_values_as_null() {
    let [before, after] = pair("Bonus", &[Some(42), None, Some(-7)]);
    let hash = Sha256::digest(&after)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        hash,
        "6a8596404b72f15482b6c9681c6e1bfde46a6c6b90bddbf314caf064ac145c04"
    );
    assert_eq!(&before[8 * 4096..10 * 4096], &after[8 * 4096..10 * 4096]);
    for offset in [16, 20, 67] {
        assert_eq!(
            &before[7 * 4096 + offset..7 * 4096 + offset + 4],
            &after[7 * 4096 + offset..7 * 4096 + offset + 4]
        );
    }
    let scope = CatalogScope::new(998);
    let mut budget = ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000);
    let table = read_table(
        &mut Cursor::new(&after),
        scope,
        "Table1",
        &mut budget,
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows(),
        &[
            vec![Value::Int32(1), Value::Int32(42), Value::Null],
            vec![Value::Int32(2), Value::Null, Value::Null],
            vec![Value::Int32(3), Value::Int32(-7), Value::Null],
        ]
    );
    assert_eq!(table.schema().columns()[2].name(), "Bonus");
    let properties = inspect_table_properties(
        &mut Cursor::new(&after),
        scope,
        "Table1",
        &mut budget,
        &|| false,
    )
    .unwrap();
    let added = properties
        .groups()
        .iter()
        .find(|group| group.name() == "Bonus")
        .unwrap();
    let default = added
        .properties()
        .iter()
        .find(|property| property.name() == "DefaultValue")
        .unwrap();
    assert_eq!(default.text().unwrap(), "0");
}

#[test]
fn future_three_field_records_preserve_historical_nulls_and_distinguish_stored_zero() {
    let [_, added] = pair("Bonus", &[Some(42), None, Some(-7)]);
    let mut current = added.to_vec();
    let mut expected = vec![
        vec![Value::Int32(1), Value::Int32(42), Value::Null],
        vec![Value::Int32(2), Value::Null, Value::Null],
        vec![Value::Int32(3), Value::Int32(-7), Value::Null],
    ];
    for (id, bonus, pin) in [
        (
            4,
            Some(0),
            "a86c8766f791a8b2b13cfadebfa0ab129a43c005115576c8851086a065aa9f03",
        ),
        (
            5,
            None,
            "19791344eadcda01e5b617853d635de1d0986f677b9a95e4b9e93bbfe9a1e126",
        ),
    ] {
        current = append_row(&current, Some(0), bonus);
        let hash = Sha256::digest(&current)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, pin);
        assert_eq!(
            &current[9 * 4096 - 33..9 * 4096],
            &added[9 * 4096 - 33..9 * 4096]
        );
        expected.push(vec![
            Value::Int32(id),
            Value::Int32(0),
            bonus.map_or(Value::Null, Value::Int32),
        ]);
        let mut budget = ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000);
        let table = read_table(
            &mut Cursor::new(&current),
            CatalogScope::new(998),
            "Table1",
            &mut budget,
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows(), expected);
    }
}
