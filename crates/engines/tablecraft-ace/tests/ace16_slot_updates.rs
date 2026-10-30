#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "ace16_encoded_delete_contract/model.rs"]
mod encoded_model;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;

mod model {
    pub use super::encoded_model::{Model, budget, capture, entries, wire};

    fn writable(mut model: Model) -> Model {
        // The shared read/DELETE model intentionally has no writable flag.
        // New slot-update fixtures opt in explicitly with the public-writer
        // metadata observed independently in both ACE16 originals.
        model.bytes[10 * 4096 + 115] = 2;
        let checked = tablecraft_ace::check_table(
            &mut std::io::Cursor::new(&model.bytes),
            tablecraft_model::CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.schema().columns()[1].flags(), 2);
        assert!(!checked.schema().columns()[1].fixed());
        model
    }

    pub fn text(family: u8, descending: bool, alias: bool) -> Model {
        writable(super::encoded_model::text(family, descending, alias))
    }

    pub fn binary(descending: bool, alias: bool) -> Model {
        writable(super::encoded_model::binary(descending, alias))
    }

    pub fn tree(model: Model, bounded: bool) -> Model {
        super::encoded_model::tree(model, bounded)
    }

    pub fn composite() -> Model {
        writable(super::encoded_model::composite())
    }
}
use tablecraft_testkit::access as support;

use model::{Model, budget, capture, entries, wire};
use std::io::Cursor;
use tablecraft_ace::{EditError, SlotScalarUpdate, WriteBlocker, check_table, read_table_index};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

fn preview(
    model: &Model,
    updates: &[SlotScalarUpdate],
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(&model.bytes).preview_ace16_slot_updates(
        "Items",
        "Score",
        updates,
        &mut budget(),
        &|| false,
    )
}

fn assert_rows(model: &Model, after: &[u8], row: usize, replacement: Value) {
    let rows = read_table_index(
        &mut Cursor::new(after),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let actual = rows
        .rows()
        .iter()
        .find(|values| values[0] == Value::Int32(row as i32 + 1))
        .unwrap();
    assert_eq!(actual[1], replacement);
    assert_eq!(rows.rows().len(), model.records.len());
    check_table(
        &mut Cursor::new(after),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        &after[10 * 4096 + 16..10 * 4096 + 20],
        &model.bytes[10 * 4096 + 16..10 * 4096 + 20]
    );
    assert_eq!(
        &after[10 * 4096 + 67..10 * 4096 + 71],
        &model.bytes[10 * 4096 + 67..10 * 4096 + 71]
    );
}

#[path = "ace16_slot_updates/boundaries.rs"]
mod boundaries;
#[path = "ace16_slot_updates/values.rs"]
mod values;

#[path = "ace16_slot_updates/properties.rs"]
mod properties;
