//! Original-only Marker bytes and current read/Actions Source; no mutation API.
use crate::{
    EditError, IndexCheckStatus, NativeImage, RuleCheckStatus, WriteBlocker, check_table,
    check_table_constraints, inspect_table_index_counter, inspect_table_properties, read_table,
    read_table_index,
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
        "h5-retained-before.accdb",
        "57c267fa365c2ff49e3ededdcfef7a6cde2f721642266c57f1ed7c7e44175398",
    ),
    (
        5,
        "batch",
        "h5-retained-batch.accdb",
        "0ae675e2f1ad8c3ad4da10b7fdb7278723ee2c0e53e6226d0d0eb1deca1fa54d",
    ),
    (
        5,
        "marker-null",
        "h5-retained-marker-null.accdb",
        "62bc4aac31a68beb4deba0ba85d7b83034c405f95c684657e33fb75e24ec94f2",
    ),
    (
        6,
        "before",
        "h6-retained-before.accdb",
        "7cd59e5aff849c5ac0567fbd69a46f575956f6bc3cb9071b2e351e8dcaf5d8fd",
    ),
    (
        6,
        "batch",
        "h6-retained-batch.accdb",
        "c5da4b88d7b55260d68e0aaf8450e8579f0d684edb2243d37683fd6b0bc25b47",
    ),
    (
        6,
        "marker-null",
        "h6-retained-marker-null.accdb",
        "14235ebea47ba225c89888a4c96e66494d477a9e8c634e5f61d85c69064dbb68",
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

fn literal(before: &[u8], data: usize, recipe: &str) -> Vec<u8> {
    let mut expected = before.to_vec();
    match recipe {
        "before" => {}
        "batch" => {
            assert_eq!(
                &before[data * 4096 + 4079 + 6..data * 4096 + 4079 + 10],
                &17_i32.to_le_bytes()
            );
            assert_eq!(
                &before[data * 4096 + 4026 + 6..data * 4096 + 4026 + 10],
                &68_i32.to_le_bytes()
            );
            expected[data * 4096 + 4085..data * 4096 + 4089].copy_from_slice(&29_i32.to_le_bytes());
            expected[data * 4096 + 4032..data * 4096 + 4036].copy_from_slice(&70_i32.to_le_bytes());
        }
        "marker-null" => {
            assert_eq!(before[data * 4096 + 4043], 7);
            expected[data * 4096 + 4043] = 3;
            assert_eq!(
                &expected[data * 4096 + 4032..data * 4096 + 4036],
                &68_i32.to_le_bytes()
            );
        }
        _ => panic!("closed source recipe"),
    }
    expected
}

fn rows(recipe: &str) -> Vec<Vec<Value>> {
    let mut rows = genuine_binary_indexed::original_rows();
    match recipe {
        "before" => {}
        "batch" => {
            rows[0][2] = Value::Int32(29);
            rows[3][2] = Value::Int32(70);
        }
        "marker-null" => rows[3][2] = Value::Null,
        _ => panic!("closed source recipe"),
    }
    rows
}

fn sources() -> Vec<(u8, &'static str, Vec<u8>)> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_RETAINED_BINARY_MARKER_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    assert_eq!(
        digest(&fs::read(directory.join("manifest.json")).unwrap()),
        "fc8f6df057d6d5a09148677a468861143975328d0c7c2d8e958fee13c5dfe1e3"
    );
    // Pin all six source views before the first native capture.
    let images = PINS
        .iter()
        .map(|&(header, recipe, file, expected)| {
            let raw = fs::read(directory.join(file)).unwrap();
            assert_eq!(raw.len(), if header == 5 { 393216 } else { 462848 });
            assert_eq!(digest(&raw), expected);
            (header, recipe, raw)
        })
        .collect::<Vec<_>>();
    assert_eq!(images.len(), 6);
    images
}

#[test]
#[ignore = "requires independently constructed Marker source corpus before UPDATE admission"]
fn retained_binary_marker_sources_have_exact_bytes_and_full_read_actions_coverage() {
    let images = sources();
    for (header, recipe, raw) in &images {
        let definition = if *header == 5 { 89 } else { 106 };
        let data = definition + 5;
        let before = &images
            .iter()
            .find(|(h, r, _)| h == header && *r == "before")
            .unwrap()
            .2;
        assert_eq!(*raw, literal(before, data, recipe));
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
            let expected = ORDERS[physical]
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
            let root = definition + 2 + physical;
            assert_eq!(
                &raw[root * 4096..(root + 1) * 4096],
                &before[root * 4096..(root + 1) * 4096]
            );
        }
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

/// Diagnostic observation only. Successful source admission is allowed to evolve;
/// the initial saved log, not a permanent refusal assertion, records history.
#[test]
#[ignore = "separate source-only canonical admission observation; not a conformance gate"]
fn observe_canonical_update_source_admission_from_pinned_originals() {
    for (header, recipe, raw) in sources() {
        if recipe != "before" {
            continue;
        }
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        let status = match normal_ordinary::capture(&image, "BinaryKeys", &mut budget(), &|| false)
        {
            Ok(_) => "ADMITTED",
            Err(EditError::Unsupported(WriteBlocker::ActiveContent)) => "REFUSED_ACTIVE_CONTENT",
            Err(EditError::Unsupported(_)) => "REFUSED_OTHER_PROFILE",
            Err(_) => "ERROR_OTHER",
        };
        println!("CANONICAL_UPDATE_SOURCE\t{header}\t{status}");
    }
}
