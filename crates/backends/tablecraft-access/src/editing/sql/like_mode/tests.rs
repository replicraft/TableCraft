//! Syntax discovery never inspects native properties for unrelated assignments.
use super::{native_mutation_query_mode, uses_like};
use std::io::Cursor;
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::database_properties;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
}

#[test]
fn criteria_detection_includes_on_and_correlated_filters_but_excludes_assignment_projection() {
    for (sql, expected) in [
        ("UPDATE Items SET Id=1 WHERE Id=2", false),
        ("UPDATE Items SET Id=IIf('a' LIKE '*',1,2)", false),
        ("UPDATE Items SET Id=1 WHERE 'a' LIKE '*'", true),
        (
            "UPDATE Items AS a INNER JOIN Items AS b ON a.Id LIKE '1*' SET a.Id=1",
            true,
        ),
        (
            "DELETE FROM Items WHERE EXISTS (SELECT Id FROM Items AS c WHERE c.Id LIKE '1*')",
            true,
        ),
        (
            "DELETE FROM Items WHERE EXISTS (SELECT Id LIKE '1*' FROM Items)",
            false,
        ),
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            uses_like(parsed.query(), &mut budget(), &|| false).unwrap(),
            expected,
            "{sql}"
        );
    }
    let original = database_properties::database(None);
    let image = NativeImage::capture(
        &mut Cursor::new(&original),
        CatalogScope::new(246),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let parsed = parse_query("UPDATE Items SET Id=1 WHERE Id=2", ParseLimits::default()).unwrap();
    assert!(
        native_mutation_query_mode(
            &image,
            parsed.query(),
            &mut ResourceBudget::new(0, 0, 0),
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .is_none(),
        "no native property allowance is needed without written criteria LIKE"
    );
}

#[test]
fn raw_mutation_observer_does_not_replace_absent_or_unknown_with_application_defaults() {
    use tablecraft_ace::DatabaseQueryMode;
    let parsed = parse_query(
        "UPDATE Items SET Id=1 WHERE 'Apple' LIKE 'a*'",
        ParseLimits::default(),
    )
    .unwrap();
    for (mode, expected) in [
        (None, DatabaseQueryMode::Absent),
        (Some(0_i32), DatabaseQueryMode::Ansi89),
        (Some(1), DatabaseQueryMode::Ansi92),
        (Some(2), DatabaseQueryMode::Unknown),
    ] {
        let bytes = mode.unwrap_or(0).to_le_bytes();
        let properties = match mode {
            Some(_) => database_properties::query_modes(&[(1, 4, &bytes)]),
            None => database_properties::query_modes(&[]),
        };
        let original = database_properties::database(Some(&properties));
        let image = NativeImage::capture(
            &mut Cursor::new(&original),
            CatalogScope::new(246),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            native_mutation_query_mode(
                &image,
                parsed.query(),
                &mut budget(),
                &mut budget(),
                &|| false
            )
            .unwrap(),
            Some(expected)
        );
    }
}
