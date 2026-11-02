#![cfg(feature = "ace16-create")]
//! Matching fresh declarations use independent empty/first-row source endpoints.
#[path = "native_creation_incrementing/components.rs"]
mod components;
#[path = "native_creation_incrementing/resources.rs"]
mod resources;

use sha2::{Digest, Sha256};
use tablecraft_ace::{
    CreateColumn, CreateDatabaseError, CreateTable, CreatedDatabase, create_ace16_database,
    create_ace16_database_tables,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::genuine_normal_autonumber as fixtures;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn sources() -> fixtures::Sources {
    fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    })
}

fn columns(reverse_builders: bool) -> [CreateColumn<'static>; 2] {
    let id = CreateColumn::new("Id", ScalarType::Int32);
    let id = if reverse_builders {
        id.with_incrementing_autonumber()
            .unwrap()
            .with_primary_key("PrimaryId")
            .unwrap()
    } else {
        id.with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap()
    };
    [id, CreateColumn::new("Score", ScalarType::Int32)]
}

fn invoke(
    multiple: bool,
    reverse_builders: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    let columns = columns(reverse_builders);
    let rows: [&[Value]; 0] = [];
    if multiple {
        create_ace16_database_tables(
            &[CreateTable::new("Items", &columns, &rows)],
            work,
            cancelled,
        )
    } else {
        create_ace16_database("Items", &columns, &rows, work, cancelled)
    }
}

#[test]
#[ignore = "requires qualified source14 before matching Increment creation"]
fn both_creators_and_builder_orders_match_the_literal_empty_increment_source() {
    let sources = sources();
    let expected = sources.bytes(5, "increment-zero-before");
    for multiple in [false, true] {
        for reverse in [false, true] {
            let created = invoke(multiple, reverse, &mut budget(), &|| false).unwrap();
            assert_eq!(created.as_bytes(), expected);
            assert_eq!(created.table_count(), 1);
            assert_eq!(created.column_count(), 2);
            assert_eq!(created.row_count(), 0);
        }
    }
}

#[test]
fn explicit_increment_declarations_refuse_wrong_types_defaults_and_nonempty_initial_rows() {
    for scalar in [
        ScalarType::Boolean,
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int64,
        ScalarType::Single,
        ScalarType::Double,
        ScalarType::Currency,
        ScalarType::DateTime,
        ScalarType::Guid,
        ScalarType::Text,
        ScalarType::Binary,
        ScalarType::Decimal,
    ] {
        assert!(matches!(
            CreateColumn::new("Id", scalar).with_incrementing_autonumber(),
            Err(CreateDatabaseError::Declaration)
        ));
    }
    let default = Value::Int32(7);
    assert!(matches!(
        CreateColumn::new("Id", ScalarType::Int32)
            .with_default_literal(&default)
            .unwrap()
            .with_incrementing_autonumber(),
        Err(CreateDatabaseError::Declaration)
    ));
    assert!(matches!(
        columns(false)[0].with_default_literal(&default),
        Err(CreateDatabaseError::Declaration)
    ));
    for id in [Value::Int32(0), Value::Int32(23), Value::Null] {
        let columns = columns(false);
        let row = [id, Value::Int32(17)];
        assert!(matches!(
            create_ace16_database("Items", &columns, &[row.as_slice()], &mut budget(), &|| {
                false
            }),
            Err(CreateDatabaseError::Declaration)
        ));
        assert!(matches!(
            create_ace16_database_tables(
                &[CreateTable::new("Items", &columns, &[row.as_slice()])],
                &mut budget(),
                &|| false
            ),
            Err(CreateDatabaseError::Declaration)
        ));
    }
    let empty: [&[Value]; 0] = [];
    let missing = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32),
    ];
    assert!(matches!(
        create_ace16_database("Items", &missing, &empty, &mut budget(), &|| false),
        Err(CreateDatabaseError::Declaration)
    ));
    let explicit_false = [
        columns(false)[0].with_required(false),
        CreateColumn::new("Score", ScalarType::Int32),
    ];
    assert!(matches!(
        create_ace16_database("Items", &explicit_false, &empty, &mut budget(), &|| false),
        Err(CreateDatabaseError::Declaration)
    ));
    let companion_false = [
        columns(false)[0],
        CreateColumn::new("Score", ScalarType::Int32).with_required(false),
    ];
    assert!(matches!(
        create_ace16_database("Items", &companion_false, &empty, &mut budget(), &|| false),
        Err(CreateDatabaseError::Declaration)
    ));
}

#[test]
fn unspecified_increment_keeps_manual_primary_and_complete_initial_values_unchanged() {
    use tablecraft_testkit::fixtures::access::canonical_creation_primary as manual;
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32),
    ];
    assert!(!columns[0].incrementing_autonumber());
    for case in [manual::Case::Empty, manual::Case::SignedUnsorted] {
        let expected = manual::source(case);
        let rows = expected.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        assert_eq!(
            create_ace16_database("Items", &columns, &rows, &mut budget(), &|| false)
                .unwrap()
                .as_bytes(),
            expected.bytes
        );
        assert_eq!(
            create_ace16_database_tables(
                &[CreateTable::new("Items", &columns, &rows)],
                &mut budget(),
                &|| false
            )
            .unwrap()
            .as_bytes(),
            expected.bytes
        );
    }
}
