//! Independent retained schemas prove tail removal across actual storage histories.
use crate::{
    EditError, InspectError, NativeImage, inspect_table_schema, read_table, read_table_projection,
    table::mutation::historical_fixed::source::Source as RowSource,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes,
    fields::{self, CASES, Case, Target},
};

const SCOPE: CatalogScope = CatalogScope::new(4103);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

#[test]
fn retained_tail_drop_preserves_rows_and_siblings_in_independent_complete_images() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                for inserted in [false, true] {
                    for extra in [false, true] {
                        let source = field_changes::source(case, target, filled, inserted, extra);
                        let native = image(&source.bytes);
                        let identity = native
                            .inspect_field_drop_source(
                                "Table1",
                                target.name(),
                                &mut budget(),
                                &|| false,
                            )
                            .unwrap();
                        let schema = inspect_table_schema(
                            &mut native.reader(),
                            SCOPE,
                            "Table1",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        let names: Vec<_> = schema
                            .columns()
                            .iter()
                            .take(schema.columns().len() - 1)
                            .map(|field| field.name())
                            .collect();
                        let projected = read_table_projection(
                            &mut native.reader(),
                            SCOPE,
                            "Table1",
                            &names,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        let expected = field_changes::drop_tail(
                            &source.bytes,
                            source.definition,
                            source.catalog_root,
                            target.name(),
                        );
                        let candidate = native
                            .prepare_retained_tail_field_drop(
                                "Table1",
                                target.name(),
                                &mut budget(),
                                &|| false,
                            )
                            .unwrap_or_else(|error| {
                                panic!("{case:?}/{target:?}/{filled}/{inserted}/{extra}: {error:?}")
                            });
                        assert!(candidate.is_retained());
                        assert_eq!(candidate.identity(), identity);
                        equal_image(candidate.before_bytes(), &source.bytes);
                        equal_image(candidate.after_bytes(), &expected);
                        let after = read_table(
                            &mut Cursor::new(candidate.after_bytes()),
                            SCOPE,
                            "Table1",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        assert_eq!(after.rows(), projected.rows());
                    }
                }
            }
        }
    }
}

#[test]
fn retained_tail_drop_repeats_across_retired_logical_numbers_and_old_rename_slack() {
    let source = fields::source(Case::FirstNumber);
    let number = fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        Target::Number,
    );
    let text = fields::after(
        &number,
        source.definition,
        source.catalog_root,
        Target::ShortText,
    );
    let expected = field_changes::rename(
        &text,
        source.definition,
        source.catalog_root,
        "NextText",
        "X",
    );
    let renamed = image(&text)
        .prepare_retained_schema_field_rename("Table1", "NextText", "X", &mut budget(), &|| false)
        .unwrap();
    equal_image(renamed.after_bytes(), &expected);
    let mut before = expected;
    for name in ["X", "NextNumber", "Score"] {
        let expected =
            field_changes::drop_tail(&before, source.definition, source.catalog_root, name);
        let candidate = image(&before)
            .prepare_retained_tail_field_drop("Table1", name, &mut budget(), &|| false)
            .unwrap_or_else(|error| panic!("drop {name}: {error:?}"));
        equal_image(candidate.after_bytes(), &expected);
        before = expected;
    }
    let retained = image(&before);
    assert!(matches!(
        RowSource::capture_retained_rows(&retained, "Table1", &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::Unsupported))
    ));
    let expected_add = fields::after(
        &before,
        source.definition,
        source.catalog_root,
        Target::Number,
    );
    let added = retained
        .prepare_retained_number_field_addition("Table1", "NextNumber", &mut budget(), &|| false)
        .unwrap();
    equal_image(added.after_bytes(), &expected_add);
    let table = read_table(
        &mut Cursor::new(before),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.columns().len(), 1);
    assert_eq!(table.rows().len(), source.roots.len());
}

#[test]
fn retained_tail_drop_keeps_legacy_and_control_boundaries_closed() {
    let source = field_changes::source(Case::FirstNumber, Target::Number, false, false, false);
    let standard = NativeImage::capture(
        &mut Cursor::new(&source.bytes),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        standard
            .prepare_current_tail_field_drop("Table1", "NextNumber", &mut budget(), &|| false)
            .is_err()
    );
    for name in ["ID", "Score", "Missing"] {
        assert!(
            image(&source.bytes)
                .prepare_retained_tail_field_drop("Table1", name, &mut budget(), &|| false)
                .is_err()
        );
    }
    assert!(
        image(&source.bytes)
            .prepare_retained_tail_field_drop("Table1", "NextNumber", &mut budget(), &|| true)
            .is_err()
    );
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(
            image(&source.bytes)
                .prepare_retained_tail_field_drop("Table1", "NextNumber", &mut allowance, &|| false)
                .is_err()
        );
    }
}
