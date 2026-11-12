//! Affirmative delete-record identity policy remains explicit through normalization.
use crate::{ParseLimits, parse_query, query::Statement};

#[test]
fn distinctrow_delete_preserves_owned_targets_joins_predicates_and_normalized_policy() {
    for sql in [
        "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id WHERE c.Score>0",
        "DELETE DISTINCTROW t.* FROM (Items AS t LEFT JOIN Changes AS c ON t.Id=c.Id) WHERE c.Id IS NULL",
        "PARAMETERS p LONG; DELETE DISTINCTROW t.* FROM Changes AS c RIGHT JOIN Items AS t ON t.Id=c.Id WHERE t.Score=p",
        "DELETE DISTINCTROW FROM Items WHERE Id=1",
    ] {
        let first = parse_query(sql, ParseLimits::default())
            .unwrap()
            .into_query();
        let Statement::Delete(delete) = &first.statement else {
            panic!("DELETE AST");
        };
        assert!(delete.distinct_row);
        let normalized = first.to_string();
        assert!(normalized.contains("DELETE DISTINCTROW "));
        let second = parse_query(&normalized, ParseLimits::default())
            .unwrap()
            .into_query();
        assert_eq!(first, second, "{normalized}");
    }
    let ordinary = parse_query(
        "DELETE t.* FROM Items AS t WHERE t.Id=1",
        ParseLimits::default(),
    )
    .unwrap()
    .into_query();
    let Statement::Delete(delete) = &ordinary.statement else {
        panic!("DELETE AST");
    };
    assert!(!delete.distinct_row);
    assert!(!ordinary.to_string().contains("DISTINCTROW"));
}

#[test]
fn distinctrow_delete_does_not_absorb_other_predicates_targets_or_trailing_statements() {
    for sql in [
        "DELETE DISTINCT DISTINCTROW t.* FROM Items AS t",
        "DELETE DISTINCTROW DISTINCTROW t.* FROM Items AS t",
        "DELETE TOP 1 t.* FROM Items AS t",
        "DELETE DISTINCTROW t.Id FROM Items AS t",
        "DELETE DISTINCTROW t.* FROM Items AS t; DELETE FROM Other",
    ] {
        assert!(parse_query(sql, ParseLimits::default()).is_err(), "{sql}");
    }
    let sql =
        "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id WHERE t.Id=1";
    assert!(
        parse_query(
            sql,
            ParseLimits {
                max_bytes: sql.len() - 1,
                ..Default::default()
            }
        )
        .is_err()
    );
}
