//! Inert Description metadata is separate from defaults and generated policies.
use super::{
    CreateColumn, CreateDatabaseError, CreateTable, FirstTableDefinition, create_ace16_database,
    create_ace16_database_tables,
};
use crate::{InspectError, NativeImage, PropertyGroupKind, inspect_table_properties};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
#[cfg(feature = "journal")]
use tablecraft_testkit::fixtures::access::canonical_add_table;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

const SCOPE: CatalogScope = CatalogScope::new(493);

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn assert_descriptions(bytes: &[u8], table: &str, expected: &[(&str, Option<&str>)]) {
    let captured = image(bytes);
    let properties =
        inspect_table_properties(&mut captured.reader(), SCOPE, table, &mut budget(), &|| {
            false
        })
        .unwrap();
    for (name, expected) in expected {
        let observed = properties
            .groups()
            .iter()
            .find(|group| group.kind() == PropertyGroupKind::Column && group.name() == *name)
            .and_then(|group| group.get("Description"));
        match (observed, expected) {
            (Some(property), Some(text)) => {
                assert_eq!(property.native_type(), 10);
                assert_eq!(property.flags(), 1);
                assert_eq!(property.text().unwrap(), *text);
                let raw = text
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>();
                assert_eq!(property.raw_value(), raw);
            }
            (None, None) => {}
            _ => panic!("Description presence differs for {name}"),
        }
    }
}

fn columns() -> [CreateColumn<'static>; 3] {
    [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_description("  café😀  ")
            .unwrap()
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::short_text("Empty", 20)
            .unwrap()
            .with_description("")
            .unwrap()
            .with_required(false)
            .with_allow_zero_length(true)
            .unwrap(),
        CreateColumn::new("Absent", ScalarType::Int32),
    ]
}

#[test]
fn descriptions_bound_utf16_units_without_trimming_or_type_restrictions() {
    let column = CreateColumn::new("Id", ScalarType::Int32);
    assert_eq!(column.description(), None);
    assert_eq!(column.with_description("").unwrap().description(), Some(""));
    for text in ["a".repeat(255), format!("{}a", "😀".repeat(127))] {
        assert_eq!(text.encode_utf16().count(), 255);
        assert_eq!(
            column.with_description(&text).unwrap().description(),
            Some(text.as_str())
        );
        let declared = [column.with_description(&text).unwrap()];
        let created =
            create_ace16_database("Items", &declared, &[], &mut budget(), &|| false).unwrap();
        assert_descriptions(created.as_bytes(), "Items", &[("Id", Some(&text))]);
    }
    for text in ["a".repeat(256), "😀".repeat(128)] {
        assert!(matches!(
            column.with_description(&text),
            Err(CreateDatabaseError::Declaration)
        ));
    }
    let primary = columns()[0];
    assert!(primary.incrementing_autonumber());
    assert_eq!(primary.description(), Some("  café😀  "));
}

#[test]
fn both_fresh_builders_keep_unicode_empty_absent_and_existing_policy_metadata() {
    let columns = columns();
    let rows: [&[Value]; 0] = [];
    let single = create_ace16_database("Items", &columns, &rows, &mut budget(), &|| false).unwrap();
    let multiple = create_ace16_database_tables(
        &[CreateTable::new("Items", &columns, &rows)],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(single.as_bytes(), multiple.as_bytes());
    assert_descriptions(
        single.as_bytes(),
        "Items",
        &[
            ("Id", Some("  café😀  ")),
            ("Empty", Some("")),
            ("Absent", None),
        ],
    );
}

#[test]
fn owned_first_definition_reborrows_descriptions_and_cancellation_remains_typed() {
    let columns = columns();
    let definition =
        FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap();
    let borrowed = definition
        .borrowed_columns(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        borrowed
            .iter()
            .map(|column| column.description())
            .collect::<Vec<_>>(),
        [Some("  café😀  "), Some(""), None]
    );
    drop(borrowed);
    let plan = image(&canonical_blank_creation::source())
        .plan_first_table(definition, &mut budget(), &|| false)
        .unwrap();
    assert_descriptions(
        plan.after_bytes(),
        "Items",
        &[
            ("Id", Some("  café😀  ")),
            ("Empty", Some("")),
            ("Absent", None),
        ],
    );
    assert!(matches!(
        FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| true),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
    ));
}

#[test]
#[cfg(feature = "journal")]
fn repeated_append_preserves_the_previous_described_table_and_its_exact_properties() {
    let original = canonical_add_table::source();
    let columns = columns();
    let added = image(&original.before)
        .plan_add_table(
            FirstTableDefinition::capture("Described", &columns, &mut budget(), &|| false).unwrap(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_descriptions(
        added.after_bytes(),
        "Described",
        &[
            ("Id", Some("  café😀  ")),
            ("Empty", Some("")),
            ("Absent", None),
        ],
    );
    let repeated = image(added.after_bytes())
        .plan_add_table(
            FirstTableDefinition::capture("Another", &columns, &mut budget(), &|| false).unwrap(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_descriptions(
        repeated.after_bytes(),
        "Described",
        &[
            ("Id", Some("  café😀  ")),
            ("Empty", Some("")),
            ("Absent", None),
        ],
    );
    assert_descriptions(
        repeated.after_bytes(),
        "Another",
        &[
            ("Id", Some("  café😀  ")),
            ("Empty", Some("")),
            ("Absent", None),
        ],
    );
    assert_eq!(
        &repeated.after_bytes()[7 * 4096..13 * 4096],
        &added.after_bytes()[7 * 4096..13 * 4096]
    );
}

#[test]
fn individually_valid_descriptions_still_refuse_aggregate_inline_overflow() {
    let names = (0..16).map(|i| format!("Field{i}")).collect::<Vec<_>>();
    let text = "x".repeat(255);
    let columns = names
        .iter()
        .map(|name| {
            CreateColumn::new(name, ScalarType::Int32)
                .with_description(&text)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        create_ace16_database("Items", &columns, &[], &mut budget(), &|| false),
        Err(CreateDatabaseError::PageCapacity)
    ));
}

#[test]
fn description_writer_matches_literal_mr2_utf16_property_bytes() {
    let columns = [CreateColumn::new("A", ScalarType::Int32)
        .with_description("é😀")
        .unwrap()];
    let created = create_ace16_database("Items", &columns, &[], &mut budget(), &|| false).unwrap();
    let captured = image(created.as_bytes());
    let properties = inspect_table_properties(
        &mut captured.reader(),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    // Independent raw MR2: one Description dictionary entry, one A column group,
    // flags 1 / Text 10, and exact é + surrogate-pair payload. No terminator.
    assert_eq!(
        properties.raw_bytes(),
        &[
            b'M', b'R', b'2', 0, 30, 0, 0, 0, 128, 0, 22, 0, b'D', 0, b'e', 0, b's', 0, b'c', 0,
            b'r', 0, b'i', 0, b'p', 0, b't', 0, b'i', 0, b'o', 0, b'n', 0, 28, 0, 0, 0, 1, 0, 8, 0,
            0, 0, 2, 0, b'A', 0, 14, 0, 1, 10, 0, 0, 6, 0, 0xe9, 0, 0x3d, 0xd8, 0, 0xde,
        ]
    );
}
