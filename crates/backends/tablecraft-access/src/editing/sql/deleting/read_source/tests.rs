use super::delete_uses_read_source;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_query::QueryError;
use tablecraft_sql::{ParseLimits, parse_query};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 100_000, 100_000)
}

#[test]
fn explicit_predicate_sources_select_the_new_mode_without_legacy_retry() {
    for (sql, expected) in [
        ("DELETE FROM Items WHERE Id=1", false),
        ("DELETE FROM Items", false),
        (
            "DELETE i.* FROM Items AS i WHERE EXISTS (SELECT Id FROM Changes WHERE Changes.Id=i.Id)",
            true,
        ),
        (
            "DELETE FROM Items WHERE IIf(TRUE, Id IN (SELECT Id FROM Changes), FALSE)",
            true,
        ),
        // These are selected, then refused by explicit source preparation.
        ("DELETE FROM Items WHERE Id=(SELECT Id FROM Changes)", true),
        (
            "DELETE FROM Items WHERE Id>ALL (SELECT Id FROM Changes)",
            true,
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            delete_uses_read_source(query.query(), &mut budget(), &|| false).unwrap(),
            expected,
            "{sql}"
        );
    }
    let query = parse_query(
        "DELETE Items.* FROM Items INNER JOIN Changes ON Items.Id=Changes.Id",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        delete_uses_read_source(query.query(), &mut budget(), &|| false),
        Err(QueryError::Unsupported)
    ));
}

#[test]
fn mode_selection_keeps_cancellation_and_work_errors_typed() {
    let query = parse_query(
        "DELETE FROM Items WHERE Id IN (SELECT Id FROM Changes)",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        delete_uses_read_source(query.query(), &mut budget(), &|| true),
        Err(QueryError::Cancelled)
    ));
    assert!(matches!(
        delete_uses_read_source(query.query(), &mut ResourceBudget::new(0, 0, 0), &|| false),
        Err(QueryError::Limit(_))
    ));
}
