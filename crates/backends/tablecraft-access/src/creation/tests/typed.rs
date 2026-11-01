use super::budget;
use crate::creation::{CreationDestination, CreationError};
use std::fs;
use tablecraft_ace::{CreateColumn, CreateDatabaseError};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::access::canonical_native17_creation as native17;
use tablecraft_testkit::access::{
    canonical_fixed_creation as fixed, canonical_fixed_float_creation as ieee,
};

fn columns(source: &fixed::Source) -> Vec<CreateColumn<'static>> {
    source
        .columns
        .iter()
        .map(|column| {
            let scalar = match column.native_type {
                1 => ScalarType::Boolean,
                2 => ScalarType::Byte,
                3 => ScalarType::Int16,
                4 => ScalarType::Int32,
                19 => ScalarType::Int64,
                5 => ScalarType::Currency,
                6 => ScalarType::Single,
                7 => ScalarType::Double,
                8 => ScalarType::DateTime,
                15 => ScalarType::Guid,
                _ => panic!("source introduced an unqualified creation declaration"),
            };
            CreateColumn::new(column.name, scalar)
        })
        .collect()
}

#[test]
fn typed_service_publishes_ten_complete_fixed_and_ieee_source_images() {
    let sources = fixed::CASES
        .into_iter()
        .map(fixed::source)
        .chain(ieee::CASES.into_iter().map(ieee::source));
    let mut observed = 0;
    for mut source in sources {
        source.bytes = native17::source(source.bytes);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("typed.accdb");
        let mut allowance = budget();
        let destination = CreationDestination::capture(&path, &mut allowance, &|| false).unwrap();
        let columns = columns(&source);
        let rows: Vec<_> = source.rows.iter().map(Vec::as_slice).collect();
        let prepared = destination
            .prepare_typed(source.table, &columns, &rows, &mut allowance, &|| false)
            .unwrap();
        assert_eq!(
            (prepared.row_count(), prepared.column_count()),
            (source.rows.len(), 10)
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
        let receipt = prepared.publish(&mut allowance, &|| false).unwrap();
        assert!(receipt.namespace_synced());
        assert_eq!(
            (receipt.row_count(), receipt.column_count()),
            (source.rows.len(), 10)
        );
        assert_eq!(fs::read(path).unwrap(), source.bytes);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        observed += 1;
    }
    assert_eq!(observed, 10);
}

#[test]
fn wrong_typed_carriers_and_empty_unsupported_declarations_create_no_output() {
    for invalid_boolean in [false, true] {
        let mut source = fixed::source(fixed::Case::Endpoints);
        source.bytes = native17::source(source.bytes);
        let declarations = columns(&source);
        let mut rows = source.rows;
        if invalid_boolean {
            rows[0][0] = Value::Null;
        } else {
            rows[0][1] = Value::Int32(0);
        }
        let rows: Vec<_> = rows.iter().map(Vec::as_slice).collect();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("invalid.accdb");
        let mut allowance = budget();
        let destination = CreationDestination::capture(&path, &mut allowance, &|| false).unwrap();
        let error = destination
            .prepare_typed(source.table, &declarations, &rows, &mut allowance, &|| {
                false
            })
            .err()
            .expect("wrong carrier must refuse before staging");
        assert!(matches!(
            error,
            CreationError::Create(CreateDatabaseError::Carrier)
        ));
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("unsupported.accdb");
    let mut allowance = budget();
    let destination = CreationDestination::capture(&path, &mut allowance, &|| false).unwrap();
    let error = destination
        .prepare_typed(
            "Items",
            &[CreateColumn::new("Text", ScalarType::Text)],
            &[],
            &mut allowance,
            &|| false,
        )
        .err()
        .expect("empty input cannot infer another type");
    assert!(matches!(
        error,
        CreationError::Create(CreateDatabaseError::UnsupportedType)
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}
