use super::*;
use tablecraft_sql::{ParseLimits, parse_query};
fn budget() -> ResourceBudget {
    ResourceBudget::new(100_000, 1000, 100_000)
}

#[test]
fn qualified_join_target_is_the_actual_table_and_exact_alias_occurrence() {
    for (sql, table, alias) in [
        (
            "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET t.Score=c.Score",
            "Items",
            "t",
        ),
        (
            "UPDATE Items AS l INNER JOIN Items AS r ON l.Id=r.Id SET r.Score=l.Score",
            "Items",
            "r",
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let target =
            local_joined_update_target(query.query(), true, &mut budget(), &|| false).unwrap();
        assert_eq!(target.table, table);
        assert_eq!(target.qualifier, alias);
    }
}

#[test]
fn mixed_unqualified_ambiguous_and_missing_targets_do_not_select_a_native_editor() {
    for (sql, error) in [
        (
            "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET Score=7",
            QueryError::Unsupported,
        ),
        (
            "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET t.Score=7,c.Score=8",
            QueryError::Unsupported,
        ),
        (
            "UPDATE Items AS t INNER JOIN Changes AS t ON t.Id=t.Id SET t.Score=7",
            QueryError::AmbiguousName,
        ),
        (
            "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET missing.Score=7",
            QueryError::UnknownName,
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
            matches!(local_joined_update_target(query.query(), true, &mut budget(), &|| false), Err(actual) if actual == error)
        );
    }
}
