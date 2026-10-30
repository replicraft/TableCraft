//! Owned statement admission is independent of source or candidate construction.
use super::{CreateIndexError, lower_create_index};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    ParseLimits, parse_query,
    query::{Query, Statement, definition::Definition},
};

fn parsed(sql: &str) -> Query {
    parse_query(sql, ParseLimits::default())
        .unwrap()
        .query()
        .clone()
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(0, 0, 100)
}

#[test]
fn ordinary_owned_shapes_borrow_the_original_table_and_accept_both_directions() {
    for sql in [
        "CREATE INDEX LookupScore ON Items (Score)",
        "CREATE INDEX LookupScore ON Items (Score ASC)",
        "CREATE INDEX [LookupScore] ON [Items] ([Score] DESC)",
    ] {
        let query = parsed(sql);
        let Statement::Definition(definition) = &query.statement else {
            panic!("owned definition");
        };
        let Definition::CreateIndex(index) = definition.as_ref() else {
            panic!("owned index");
        };

        let mut work = ResourceBudget::new(0, 0, 1);
        let (table, _request) = lower_create_index(&query, &mut work, &|| false).unwrap();
        assert_eq!(table, "Items");
        assert_eq!(table.as_ptr(), index.table.as_str().as_ptr());
        assert_eq!(work.remaining(Resource::Steps), 0);
    }
}

#[test]
fn unsupported_owned_options_and_actions_never_become_native_requests() {
    for sql in [
        "SELECT 1",
        "CREATE TABLE Items (Id LONG)",
        "DROP INDEX LookupScore ON Items",
        "CREATE UNIQUE INDEX LookupScore ON Items (Score)",
        "CREATE INDEX LookupScore ON Items (Score) WITH PRIMARY",
        "CREATE INDEX LookupScore ON Items (Score) WITH DISALLOW NULL",
        "CREATE INDEX LookupScore ON Items (Score) WITH IGNORE NULL",
        "CREATE INDEX LookupScore ON Items (Score, Id)",
    ] {
        assert!(matches!(
            lower_create_index(&parsed(sql), &mut budget(), &|| false),
            Err(CreateIndexError::Unsupported)
        ));
    }

    let mut query = parsed("CREATE INDEX LookupScore ON Items (Score)");
    let Statement::Definition(definition) = &mut query.statement else {
        panic!("owned definition");
    };
    let Definition::CreateIndex(index) = definition.as_mut() else {
        panic!("owned index");
    };
    index.fields.clear();
    assert!(matches!(
        lower_create_index(&query, &mut budget(), &|| false),
        Err(CreateIndexError::Unsupported)
    ));

    let mut parameterized = parsed("PARAMETERS p LONG; SELECT p");
    parameterized.statement = parsed("CREATE INDEX LookupScore ON Items (Score)").statement;
    assert!(matches!(
        lower_create_index(&parameterized, &mut budget(), &|| false),
        Err(CreateIndexError::Unsupported)
    ));
}

#[test]
fn constant_shape_work_and_cancellation_are_checked_without_a_source() {
    let query = parsed("CREATE INDEX LookupScore ON Items (Score)");
    let mut short = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        lower_create_index(&query, &mut short, &|| false),
        Err(CreateIndexError::Inspect(InspectError::Limit(error)))
            if error.resource == Resource::Steps
    ));
    let mut cancelled = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        lower_create_index(&query, &mut cancelled, &|| true),
        Err(CreateIndexError::Inspect(InspectError::Cancelled))
    ));
}
