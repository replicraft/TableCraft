//! A real populated middle declaration remains independently addressable by native ID.
use crate::{NativeImage, inspect_table_schema, read_table};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, fields, typed_fields,
};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(4405);

fn budget() -> ResourceBudget {
    ResourceBudget::new(100_000_000, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn retained_middle_field_drop_admits_populated_score_without_moving_surviving_amount() {
    let source = fields::source(fields::Case::FirstNumber);
    let added = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Amount",
        typed_fields::Kind::Currency,
    );
    let before = typed_fields::update_currency(&added, source.definition, 0, Some(12345));
    assert_eq!(
        read_table(
            &mut Cursor::new(&before),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        [
            vec![
                Value::Int32(1),
                Value::Int32(17),
                Value::Currency(Currency::from_scaled(12345))
            ],
            vec![Value::Int32(2), Value::Null, Value::Null],
        ]
    );

    // The current tail action qualifies the same source, proving the intended
    // baseline failure is the middle target rather than unrelated source storage.
    image(&before)
        .prepare_retained_typed_tail_field_drop("Table1", "Amount", &mut budget(), &|| false)
        .unwrap();

    let expected =
        field_changes::drop_field(&before, source.definition, source.catalog_root, "Score");
    let native = image(&expected);
    let schema = inspect_table_schema(
        &mut native.reader(),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        schema
            .columns()
            .iter()
            .map(|column| (column.name(), column.number()))
            .collect::<Vec<_>>(),
        [("ID", 0), ("Amount", 2)]
    );
    assert_eq!(
        read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        [
            vec![
                Value::Int32(1),
                Value::Currency(Currency::from_scaled(12345))
            ],
            vec![Value::Int32(2), Value::Null],
        ]
    );
    let definition = source.definition as usize;
    assert_eq!(
        &expected[definition * PAGE + 41..definition * PAGE + 45],
        &before[definition * PAGE + 41..definition * PAGE + 45]
    );
    for page in 0..before.len() / PAGE {
        // Only actual definition/catalog/property snapshot containers change.
        if ![4, 10, definition].contains(&page) {
            assert_eq!(
                &expected[page * PAGE..(page + 1) * PAGE],
                &before[page * PAGE..(page + 1) * PAGE],
                "protected page {page}"
            );
        }
    }

    // The normal inspection entry point currently refuses this valid middle
    // target. The next action must qualify it without widening tail families.
    let identity = image(&before)
        .inspect_field_drop_source("Table1", "Score", &mut budget(), &|| false)
        .expect("ordinary middle field must have a checked DROP source");
    assert_eq!(identity.column_number(), 1);
    assert_eq!(identity.definition_page(), source.definition);
}
