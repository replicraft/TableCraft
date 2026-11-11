//! A real populated middle declaration remains independently addressable by native ID.
use crate::{
    EditError, FieldDropError, InspectError, NativeImage, inspect_table_properties,
    inspect_table_schema, read_table,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
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

fn populated() -> (fields::Source, Vec<u8>) {
    let source = fields::source(fields::Case::FirstNumber);
    let added = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Amount",
        typed_fields::Kind::Currency,
    );
    let before = typed_fields::update_currency(&added, source.definition, 0, Some(12345));
    (source, before)
}

#[test]
fn retained_middle_field_drop_admits_populated_score_without_moving_surviving_amount() {
    let (source, before) = populated();
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

    // Explicit new-language inspection/candidate creation preserves the old
    // tail routes. Generic host availability remains closed until wire support.
    let identity = image(&before)
        .inspect_retained_non_tail_field_drop_source("Table1", "Score", &mut budget(), &|| false)
        .expect("ordinary middle field must have a checked DROP source");
    assert_eq!(identity.column_number(), 1);
    assert_eq!(identity.definition_page(), source.definition);
    let candidate = image(&before)
        .prepare_retained_non_tail_field_drop("Table1", "Score", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.identity(), identity);
    assert_eq!(candidate.table(), "Table1");
    assert_eq!(candidate.column(), "Score");
    assert_eq!(candidate.declared_rows(), 2);
    assert!(candidate.before_bytes() == before);
    assert_eq!(candidate.after_bytes().len(), expected.len());
    assert!(
        candidate.after_bytes() == expected,
        "first difference {:?}",
        candidate
            .after_bytes()
            .iter()
            .zip(&expected)
            .position(|(actual, expected)| actual != expected)
    );
}

#[test]
fn retained_middle_language_keeps_tail_primary_and_generic_host_routes_closed() {
    let (_, before) = populated();
    let native = image(&before);
    assert!(
        native
            .select_native_retained_non_tail_field_drop_source(
                "Table1",
                "Score",
                &mut budget(),
                &|| false
            )
            .unwrap()
    );
    assert!(
        !native
            .select_native_retained_non_tail_field_drop_source(
                "Table1",
                "Amount",
                &mut budget(),
                &|| false
            )
            .unwrap()
    );
    assert!(matches!(
        native.inspect_field_drop_source("Table1", "Score", &mut budget(), &|| false),
        Err(FieldDropError::Source)
    ));
    assert!(matches!(
        image(&before)
            .prepare_retained_tail_field_drop("Table1", "Score", &mut budget(), &|| false),
        Err(FieldDropError::Source)
    ));
    assert!(matches!(
        image(&before).prepare_retained_typed_tail_field_drop(
            "Table1",
            "Score",
            &mut budget(),
            &|| false
        ),
        Err(FieldDropError::Source)
    ));
    for target in ["ID", "Amount"] {
        assert!(
            matches!(
                image(&before).prepare_retained_non_tail_field_drop(
                    "Table1",
                    target,
                    &mut budget(),
                    &|| false
                ),
                Err(FieldDropError::Source)
            ),
            "target {target}"
        );
    }
}

fn inspection(error: &FieldDropError) -> Option<&InspectError> {
    match error {
        FieldDropError::Inspect(error) | FieldDropError::Placement(EditError::Inspect(error)) => {
            Some(error)
        }
        _ => None,
    }
}

#[test]
fn retained_middle_source_rejects_unknown_properties_and_preserves_control_failures() {
    let (_, before) = populated();
    let native = image(&before);
    let properties = inspect_table_properties(
        &mut native.reader(),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let body = properties.raw_bytes();
    let body_start = before
        .windows(body.len())
        .rposition(|window| window == body)
        .unwrap();
    let required = "Required"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let key = body
        .windows(required.len())
        .position(|window| window == required)
        .unwrap();
    let mut unknown = before.clone();
    unknown[body_start + key] = b'X';
    let changed = image(&unknown);
    let observed = inspect_table_properties(
        &mut changed.reader(),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(observed.groups().len(), properties.groups().len());
    assert!(matches!(
        changed.prepare_retained_non_tail_field_drop("Table1", "Score", &mut budget(), &|| false),
        Err(FieldDropError::Source | FieldDropError::Placement(EditError::Unsupported(_)))
    ));
    for (mut work, resource) in [
        (
            ResourceBudget::new(0, 100_000, 100_000_000),
            Resource::Bytes,
        ),
        (
            ResourceBudget::new(100_000_000, 0, 100_000_000),
            Resource::Rows,
        ),
        (
            ResourceBudget::new(100_000_000, 100_000, 0),
            Resource::Steps,
        ),
    ] {
        let error = image(&before)
            .prepare_retained_non_tail_field_drop("Table1", "Score", &mut work, &|| false)
            .err()
            .expect("caller exhaustion must refuse");
        assert!(
            matches!(inspection(&error), Some(InspectError::Limit(limit)) if limit.resource == resource)
        );
    }
    let error = image(&before)
        .prepare_retained_non_tail_field_drop("Table1", "Score", &mut budget(), &|| true)
        .err()
        .expect("cancellation must refuse");
    assert!(matches!(inspection(&error), Some(InspectError::Cancelled)));
}
