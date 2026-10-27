#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "ace16_fixed_split_contract/model.rs"]
mod fixed_model;
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "ace16_split_int32_contract/model.rs"]
mod root_model;
#[allow(dead_code)]
mod support;

#[path = "ace16_index_map_growth_contract/authority.rs"]
mod authority;
#[path = "ace16_index_map_growth_contract/late.rs"]
mod late;
#[path = "ace16_index_map_growth_contract/model.rs"]
mod model;
#[path = "ace16_index_map_growth_contract/proof.rs"]
mod proof;
#[path = "ace16_index_map_growth_contract/values.rs"]
mod values;

use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    Ace16WritePreview, EditError, FixedScalarUpdate, IndexCheckStatus, InspectError, NativeImage,
    WriteBlocker, check_table, check_table_constraints, check_table_long_values, read_table,
    read_table_index,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

fn request(value: i32) -> FixedScalarUpdate {
    FixedScalarUpdate::new(0, Value::Int32(0), Value::Int32(value))
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut fixed_model::budget(),
        &|| false,
    )
    .unwrap()
}

fn run(model: &model::Model) -> Result<Ace16WritePreview, EditError> {
    capture(&model.bytes).preview_ace16_split_fixed_updates(
        "Items",
        "Score",
        &[request(i32::MAX)],
        &mut fixed_model::budget(),
        &|| false,
    )
}

fn before(model: &model::Model, logical_indexes: usize) {
    for (name, count) in [
        ("MSysObjects", 4),
        ("MSysRelationships", 0),
        ("Items", model::ROWS),
        ("Other", 0),
    ] {
        let table = read_table(
            &mut Cursor::new(&model.bytes),
            CatalogScope::new(1),
            name,
            &mut fixed_model::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows().len(), count);
        let indexes = check_table(
            &mut Cursor::new(&model.bytes),
            CatalogScope::new(1),
            name,
            &mut fixed_model::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            indexes.indexes().len(),
            if name == "Items" { logical_indexes } else { 0 }
        );
        assert!(
            indexes
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        let constraints = check_table_constraints(
            &mut Cursor::new(&model.bytes),
            CatalogScope::new(1),
            name,
            &mut fixed_model::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(constraints.rows(), count);
        assert!(constraints.relationships().is_empty());
        check_table_long_values(
            &mut Cursor::new(&model.bytes),
            CatalogScope::new(1),
            name,
            &mut fixed_model::budget(),
            &|| false,
        )
        .unwrap();
    }
}

fn verify(model: &model::Model, result: &Ace16WritePreview) {
    assert_eq!(result.changed_rows(), 1);
    assert_eq!(result.candidate_bytes(), proof::expected(model));
    let bytes = result.candidate_bytes();
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut fixed_model::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), model::ROWS);
    for (row, values) in table.rows().iter().enumerate() {
        assert_eq!(values[0], Value::Int32(row as i32 + 1));
        assert_eq!(values[1], Value::Int32(if row == 0 { i32::MAX } else { 0 }));
    }
    for name in ["ScoreKey", "ScoreAlias", "ReverseKey", "ReverseAlias"] {
        let index = read_table_index(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            name,
            &mut fixed_model::budget(),
            &|| false,
        )
        .unwrap();
        let mut identities = (2..=model::ROWS as i32).collect::<Vec<_>>();
        if name.starts_with("Reverse") {
            identities.insert(0, 1);
        } else {
            identities.push(1);
        }
        assert_eq!(index.rows().len(), identities.len());
        for (row, identity) in index.rows().iter().zip(identities) {
            assert_eq!(row[0], Value::Int32(identity));
        }
    }
    for name in ["Items", "Other"] {
        let report = check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            name,
            &mut fixed_model::budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            report
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            name,
            &mut fixed_model::budget(),
            &|| false,
        )
        .unwrap();
    }
}
