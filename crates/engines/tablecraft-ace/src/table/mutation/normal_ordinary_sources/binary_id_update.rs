//! Independent I32 primary replacement; complete read and Source qualification only.
use crate::{
    IndexCheckStatus, NativeImage, RuleCheckStatus, check_table, check_table_constraints,
    inspect_table_index_counter, inspect_table_properties, read_table, read_table_index,
    table::mutation::{normal_actions, normal_ordinary},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_binary_indexed;

const SCOPE: CatalogScope = CatalogScope::new(209);
const PINS: &[(u8, &str, &str, &str)] = &[
    (
        5,
        "before",
        "h5-binary-id-before.accdb",
        "57c267fa365c2ff49e3ededdcfef7a6cde2f721642266c57f1ed7c7e44175398",
    ),
    (
        5,
        "id-14",
        "h5-binary-id-id-14.accdb",
        "4c25b772e1fb98e6d5de7977117d03f7dca7f4b2d4e7030669925828400fcb5e",
    ),
    (
        6,
        "before",
        "h6-binary-id-before.accdb",
        "7cd59e5aff849c5ac0567fbd69a46f575956f6bc3cb9071b2e351e8dcaf5d8fd",
    ),
    (
        6,
        "id-14",
        "h6-binary-id-id-14.accdb",
        "740326003559f5b21913ed802867eca5d25c38dbe0247fa602767ed6bddf2f14",
    ),
];
const ALIASES: [&str; 6] = [
    "PrimaryID",
    "PrimaryAlias",
    "Ascending",
    "AscendingAlias",
    "Descending",
    "DescendingAlias",
];
// Exact original saved SDK cursor ROW associations, not decoder/writer output.
const ORDERS: [[usize; 13]; 3] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
    [0, 1, 2, 12, 4, 10, 5, 6, 7, 8, 9, 3, 11],
    [1, 11, 3, 9, 8, 7, 6, 5, 10, 4, 2, 12, 0],
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

fn literal(before: &[u8], definition: usize, data: usize, recipe: &str) -> Vec<u8> {
    let mut expected = before.to_vec();
    let root = (definition + 2) * 4096;
    let mut bitmap = [0_u8; 453];
    for end in (9..=117).step_by(9) {
        bitmap[end / 8] |= 1 << (end % 8);
    }
    assert_eq!(&before[root + 27..root + 480], &bitmap);
    assert_eq!(
        u16::from_le_bytes(before[root + 2..root + 4].try_into().unwrap()),
        3499
    );
    assert_eq!(&before[root + 8..root + 27], &[0; 19]);
    assert_eq!(
        &before[data * 4096 + 4081..data * 4096 + 4085],
        &1_i32.to_le_bytes()
    );
    for slot in 0..13_usize {
        let at = root + 480 + slot * 9;
        assert_eq!(before[at], 0x7f);
        assert_eq!(
            &before[at + 1..at + 5],
            &((slot as u32 + 1) ^ 0x8000_0000).to_be_bytes()
        );
        assert_eq!(
            &before[at + 5..at + 9],
            &(((data as u32) << 8) | slot as u32).to_be_bytes()
        );
    }
    if recipe == "id-14" {
        expected[data * 4096 + 4081..data * 4096 + 4085].copy_from_slice(&14_i32.to_le_bytes());
        for (position, slot) in (1..13_usize).chain(std::iter::once(0)).enumerate() {
            let identity = if slot == 0 { 14 } else { slot as u32 + 1 };
            let at = root + 480 + position * 9;
            expected[at] = 0x7f;
            expected[at + 1..at + 5].copy_from_slice(&(identity ^ 0x8000_0000).to_be_bytes());
            expected[at + 5..at + 9]
                .copy_from_slice(&(((data as u32) << 8) | slot as u32).to_be_bytes());
        }
    } else {
        assert_eq!(recipe, "before");
    }
    expected
}

fn rows(recipe: &str) -> Vec<Vec<Value>> {
    let mut rows = genuine_binary_indexed::original_rows();
    if recipe == "id-14" {
        rows[0][0] = Value::Int32(14);
    } else {
        assert_eq!(recipe, "before");
    }
    rows
}

fn sources() -> Vec<(u8, &'static str, Vec<u8>)> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_BINARY_ID_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    assert_eq!(
        digest(&fs::read(directory.join("manifest.json")).unwrap()),
        "aa190d73441d1eea75a995402afc3dc9d6fc65f39e314f0ed04d55a09e57f37b"
    );
    // Pin all four source views before the first native capture.
    let images = PINS
        .iter()
        .map(|&(header, recipe, file, expected)| {
            let raw = fs::read(directory.join(file)).unwrap();
            assert_eq!(raw.len(), if header == 5 { 393216 } else { 462848 });
            assert_eq!(digest(&raw), expected);
            (header, recipe, raw)
        })
        .collect::<Vec<_>>();
    assert_eq!(images.len(), 4);
    images
}

#[test]
#[ignore = "requires independently constructed Binary Id sources; no writer calls"]
fn binary_id_sources_have_exact_primary_membership_and_full_source() {
    let images = sources();
    for (header, recipe, raw) in &images {
        let definition = if *header == 5 { 89 } else { 106 };
        let data = definition + 5;
        let before = &images
            .iter()
            .find(|(h, r, _)| h == header && *r == "before")
            .unwrap()
            .2;
        assert_eq!(*raw, literal(before, definition, data, recipe));
        let expected_rows = rows(recipe);
        let image =
            NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
        let table = read_table(
            &mut image.reader(),
            SCOPE,
            "BinaryKeys",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows(), expected_rows.as_slice());
        let schema = table.schema();
        assert_eq!(schema.definition_page() as usize, definition);
        assert_eq!(schema.columns().len(), 3);
        let id = &schema.columns()[0];
        assert_eq!(id.name(), "Id");
        assert_eq!(id.number(), 0);
        assert_eq!(id.native_type(), 4);
        assert_eq!(id.flags(), 3);
        assert_eq!(id.extended_flags(), 0);
        assert_eq!(id.storage_length(), 4);
        assert!(id.fixed());
        assert_eq!(schema.columns()[2].name(), "Marker");
        assert_eq!(schema.columns()[2].flags(), 3);
        assert_eq!(schema.columns()[2].extended_flags(), 0);
        assert_eq!(schema.columns()[2].native_type(), 4);
        assert!(schema.columns()[2].fixed());
        assert_eq!(schema.columns()[2].number(), 2);
        assert_eq!(schema.columns()[2].field.offset, 4);
        assert_eq!(schema.columns()[2].storage_length(), 4);
        let properties = inspect_table_properties(
            &mut image.reader(),
            SCOPE,
            "BinaryKeys",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(properties.groups().is_empty());
        assert!(properties.raw_bytes().is_empty());
        let checked = check_table(
            &mut image.reader(),
            SCOPE,
            "BinaryKeys",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 13);
        assert_eq!(checked.indexes().len(), 6);
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        let constraints = check_table_constraints(
            &mut image.reader(),
            SCOPE,
            "BinaryKeys",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let marker = constraints
            .fields()
            .iter()
            .find(|field| field.name() == "Marker")
            .unwrap();
        assert_eq!(marker.required(), RuleCheckStatus::NotDeclared);
        assert_eq!(marker.validation(), RuleCheckStatus::NotDeclared);
        assert!(constraints.relationships().is_empty());
        for (ordinal, alias) in ALIASES.iter().enumerate() {
            let physical = ordinal / 2;
            let indexed = read_table_index(
                &mut image.reader(),
                SCOPE,
                "BinaryKeys",
                alias,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let order = if physical == 0 && *recipe == "id-14" {
                [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0]
            } else {
                ORDERS[physical]
            };
            let expected = order
                .iter()
                .map(|&row| expected_rows[row].clone())
                .collect::<Vec<_>>();
            assert_eq!(indexed.rows(), expected.as_slice());
            let counter = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                "BinaryKeys",
                alias,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(counter.value(), if physical == 0 { 13 } else { 12 });
            if physical != 0 {
                let root = definition + 2 + physical;
                assert_eq!(
                    &raw[root * 4096..(root + 1) * 4096],
                    &before[root * 4096..(root + 1) * 4096]
                );
            }
        }
        let canonical =
            normal_ordinary::capture(&image, "BinaryKeys", &mut budget(), &|| false).unwrap();
        assert_eq!(canonical.table.rows(), expected_rows.as_slice());
        let action_source =
            normal_actions::capture(&image, "BinaryKeys", &mut budget(), &|| false).unwrap();
        assert_eq!(action_source.table.rows(), expected_rows.as_slice());
        assert_eq!(action_source.records.len(), 13);
        assert_eq!(before[data * 4096 + 4095], 5);
        assert_eq!(before[data * 4096 + 4043], 7);
        for (ordinal, record) in action_source.records.iter().enumerate() {
            assert_eq!(record.reference, ((data as u32) << 8) | ordinal as u32);
        }
    }
}
