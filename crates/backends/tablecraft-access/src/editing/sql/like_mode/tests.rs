//! Syntax discovery never inspects native properties for unrelated assignments.
use super::{native_mutation_like_mode, uses_like};
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
        native_mutation_like_mode(
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
