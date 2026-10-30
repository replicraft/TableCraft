//! Complete explicit requests retain inert default declarations byte for byte.
mod api;
mod model;

use super::{
    incrementing_autonumber::model as generated,
    model as original,
    ordinary_rows::{fixture, model::Row},
};
use crate::{RuleCheckStatus, check_table_constraints, inspect_table_properties, read_table};
use model::{Group, PAGE, Property, field};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

fn check(bytes: &[u8], rows: usize, aliases: usize) {
    original::checked(bytes, rows, aliases);
    let report = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(report.table_validation(), RuleCheckStatus::Unsupported);
    for field in report.fields() {
        assert!(
            ![field.required(), field.empty_text(), field.validation()]
                .contains(&RuleCheckStatus::Unsupported)
        );
    }
}

fn negative_sources() -> Vec<(Vec<u8>, bool)> {
    let mut result = Vec::new();
    for group in [
        Group {
            kind: 0,
            name: "",
            properties: vec![Property::text("DefaultValue", "0")],
        },
        Group {
            kind: 2,
            name: "PrimaryID",
            properties: vec![Property::text("DefaultValue", "0")],
        },
        Group {
            kind: 7,
            name: "Score",
            properties: vec![Property::text("DefaultValue", "0")],
        },
        field("Unknown", vec![Property::text("DefaultValue", "0")]),
        field("Score", vec![Property::boolean("DefaultValue", false)]),
        field(
            "Score",
            vec![Property {
                flags: 2,
                ..Property::text("DefaultValue", "0")
            }],
        ),
    ] {
        let mut case = fixture::source(6, true, false, fixture::Index::Alias, false);
        model::properties(&mut case.before, &[group]);
        result.push((case.before, false));
    }
    let mut generated = generated::source(6, generated::State::Fresh);
    model::properties(
        &mut generated.before,
        &[field("ID", vec![Property::text("DefaultValue", "0")])],
    );
    result.push((generated.before, true));
    let mut mode = generated::source(6, generated::State::Fresh);
    model::properties(
        &mut mode.before,
        &[
            field("Score", vec![Property::text("DefaultValue", "0")]),
            field("ID", vec![Property::text("NewValues", "Random")]),
        ],
    );
    result.push((mode.before, true));
    let mut ole =
        super::long_value_inserts::external::original(6, super::long_value_inserts::Family::Ole);
    model::properties(
        &mut ole.before,
        &[field("Score", vec![Property::text("DefaultValue", "Null")])],
    );
    result.push((ole.before, false));
    result
}

#[test]
fn qualify_passive_default_sources_and_complete_explicit_expected_images_before_api() {
    for code in [5, 6] {
        for expression in [
            "\"stored default\"",
            "Date()",
            "UnresolvedDefaultFunction()",
        ] {
            let case = model::ordinary(code, expression, false);
            let additions = model::additions();
            let expected = fixture::expected(&case, &additions);
            check(&case.before, 3, 2);
            check(&expected, 6, 2);
            assert_eq!(
                &expected[4 * PAGE..5 * PAGE],
                &case.before[4 * PAGE..5 * PAGE]
            );
            assert_eq!(fixture::expected(&case, &[]), case.before);
            let snapshot = read_table(
                &mut Cursor::new(&expected),
                CatalogScope::new(1),
                "Items",
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                &snapshot.rows()[3..],
                additions
                    .iter()
                    .map(|row| row.values(true))
                    .collect::<Vec<_>>()
            );
            let required = model::ordinary(code, expression, true);
            check(&required.before, 3, 2);
        }
        for allow_empty in [false, true] {
            let rules = model::rules(code, allow_empty);
            check(&rules.before, 3, 2);
            let expected = fixture::expected(&rules, &[Row::rich(10)]);
            check(&expected, 4, 2);
        }
        let mut case = generated::source(code, generated::State::HighWater);
        model::properties(
            &mut case.before,
            &[field("Score", vec![Property::text("DefaultValue", "999")])],
        );
        let additions = [generated::Row {
            earlier: 4,
            id: 261,
            score: None,
        }];
        let expected = generated::expected(&case, &additions);
        check(&case.before, 2, 4);
        check(&expected, 3, 4);
        assert_eq!(
            &expected[4 * PAGE..5 * PAGE],
            &case.before[4 * PAGE..5 * PAGE]
        );
    }
    for (bytes, _) in negative_sources() {
        // These are readable stored declarations whose write semantics are
        // deliberately refused, rather than malformed record sources.
        read_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
        let properties = inspect_table_properties(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            properties
                .groups()
                .iter()
                .any(|group| group.get("DefaultValue").is_some())
        );
    }
}
