//! Retained rename changes only bound declaration/group names and snapshot publication.
use crate::{NativeImage, read_table};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes,
    fields::{self, CASES, Case, Target},
};

const SCOPE: CatalogScope = CatalogScope::new(4206);

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
fn retained_schema_renames_match_raw_grow_shrink_case_and_noop_images() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                for inserted in [false, true] {
                    for extra in [false, true] {
                        let source = field_changes::source(case, target, filled, inserted, extra);
                        let lower = target.name().to_ascii_lowercase();
                        for name in [
                            "AConsiderablyLongerName",
                            "X",
                            lower.as_str(),
                            target.name(),
                        ] {
                            let native = image(&source.bytes);
                            let facts = native
                                .inspect_retained_schema_field_rename_source(
                                    "Table1",
                                    target.name(),
                                    &mut budget(),
                                    &|| false,
                                )
                                .unwrap();
                            let expected = field_changes::rename(
                                &source.bytes,
                                source.definition,
                                source.catalog_root,
                                target.name(),
                                name,
                            );
                            let old = read_table(
                                &mut native.reader(),
                                SCOPE,
                                "Table1",
                                &mut budget(),
                                &|| false,
                            )
                            .unwrap();
                            let candidate = native.prepare_retained_schema_field_rename("Table1", target.name(), name, &mut budget(), &|| false).unwrap_or_else(|error| panic!("{case:?}/{target:?}/{filled}/{inserted}/{extra}/{name}: {error:?}"));
                            assert!(candidate.is_retained_schema());
                            assert_eq!(candidate.identity(), facts.identity());
                            assert_eq!(candidate.changed(), name != target.name());
                            equal_image(candidate.before_bytes(), &source.bytes);
                            equal_image(candidate.after_bytes(), &expected);
                            let current = read_table(
                                &mut Cursor::new(candidate.after_bytes()),
                                SCOPE,
                                "Table1",
                                &mut budget(),
                                &|| false,
                            )
                            .unwrap();
                            assert_eq!(current.rows(), old.rows());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn retained_schema_rename_handles_original_inline_properties_and_typed_refusals() {
    for (case, field) in [(Case::FirstNumber, "Score"), (Case::SecondText, "Name")] {
        let source = fields::source(case);
        for name in ["LongerNativeLabel", "X", field] {
            let expected = field_changes::rename(
                &source.bytes,
                source.definition,
                source.catalog_root,
                field,
                name,
            );
            let candidate = image(&source.bytes)
                .prepare_retained_schema_field_rename("Table1", field, name, &mut budget(), &|| {
                    false
                })
                .unwrap();
            equal_image(candidate.after_bytes(), &expected);
        }
    }
    let source = field_changes::source(Case::FirstNumber, Target::ShortText, true, true, false);
    let standard = NativeImage::capture(
        &mut Cursor::new(&source.bytes),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        standard
            .prepare_retained_field_rename("Table1", "NextText", "Changed", &mut budget(), &|| {
                false
            })
            .is_err()
    );
    for name in ["", "ID", "日本語"] {
        assert!(
            image(&source.bytes)
                .prepare_retained_schema_field_rename(
                    "Table1",
                    "NextText",
                    name,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
    assert!(
        image(&source.bytes)
            .prepare_retained_schema_field_rename(
                "Table1",
                "NextText",
                "NextText",
                &mut budget(),
                &|| true
            )
            .is_err()
    );
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(
            image(&source.bytes)
                .prepare_retained_schema_field_rename(
                    "Table1",
                    "NextText",
                    "NextText",
                    &mut allowance,
                    &|| false
                )
                .is_err()
        );
    }
}
