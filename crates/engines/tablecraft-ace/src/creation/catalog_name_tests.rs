//! Catalog text and raw definition names against independent source images.
use super::{
    CreateColumn, CreateDatabaseError, create_ace16_database, create_ace16_int32_database,
};
use crate::read_table;
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_catalog_names;
use tablecraft_testkit::fixtures::access::canonical_native17_creation as native17;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 16_000_000)
}

#[test]
fn catalog_names_match_four_complete_independent_sources_through_both_apis() {
    for case in canonical_catalog_names::CASES {
        let mut source = canonical_catalog_names::source(case);
        source.bytes = native17::source(source.bytes);
        let columns = source.columns.each_ref().map(String::as_str);
        let declarations = columns.map(|name| CreateColumn::new(name, ScalarType::Int32));
        let typed =
            create_ace16_database(&source.table, &declarations, &[], &mut budget(), &|| false)
                .unwrap();
        let plain =
            create_ace16_int32_database(&source.table, &columns, &[], &mut budget(), &|| false)
                .unwrap();
        assert_eq!(typed.as_bytes(), source.bytes, "{}", case.id());
        assert_eq!(plain.as_bytes(), source.bytes, "{}", case.id());
        assert_eq!(typed.row_count(), 0);
        assert_eq!(plain.column_count(), 2);
    }
}

#[test]
fn catalog_escape_preserves_name_while_definition_names_remain_raw() {
    for table in [
        "\u{feff}",
        "\u{feff}A",
        "\u{feff}\u{0101}",
        "\u{feff}🙂",
        "A\u{feff}\u{0100}",
    ] {
        let column = "\u{feff}\u{0100}";
        let values = [Value::Int32(42)];
        let declarations = [CreateColumn::new(column, ScalarType::Int32)];
        let typed =
            create_ace16_database(table, &declarations, &[&values], &mut budget(), &|| false)
                .unwrap();
        let plain =
            create_ace16_int32_database(table, &[column], &[&[42]], &mut budget(), &|| false)
                .unwrap();
        assert_eq!(typed.as_bytes(), plain.as_bytes());
        let read = read_table(
            &mut Cursor::new(typed.as_bytes()),
            CatalogScope::new(1),
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(read.schema().columns()[0].name(), column);
        assert_eq!(read.rows(), [values.to_vec()]);
    }
}

#[test]
fn unsupported_catalog_encoding_refuses_before_image_reservation() {
    let table = "\u{feff}\u{0100}";
    for populated in [false, true] {
        let value = [Value::Int32(42)];
        let typed_rows = if populated {
            vec![value.as_slice()]
        } else {
            vec![]
        };
        let plain_value = [42_i32];
        let plain_rows = if populated {
            vec![plain_value.as_slice()]
        } else {
            vec![]
        };
        let mut typed_budget = ResourceBudget::new(0, 100_000, 16_000_000);
        let mut plain_budget = ResourceBudget::new(0, 100_000, 16_000_000);
        assert!(matches!(
            create_ace16_database(
                table,
                &[CreateColumn::new("Id", ScalarType::Int32)],
                &typed_rows,
                &mut typed_budget,
                &|| false
            ),
            Err(CreateDatabaseError::TextEncoding)
        ));
        assert!(matches!(
            create_ace16_int32_database(table, &["Id"], &plain_rows, &mut plain_budget, &|| false),
            Err(CreateDatabaseError::TextEncoding)
        ));
        assert_eq!(typed_budget.remaining(Resource::Bytes), 0);
        assert_eq!(plain_budget.remaining(Resource::Bytes), 0);
    }
}
