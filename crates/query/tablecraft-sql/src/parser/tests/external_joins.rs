//! FROM database selection must remain distinct from ON membership predicates.
use super::*;
use crate::select::{JoinKind, TableSource};

#[test]
fn unparenthesized_join_external_sources_preserve_join_kind_and_condition() {
    for (syntax, kind) in [
        ("INNER", JoinKind::Inner),
        ("LEFT", JoinKind::Left),
        ("RIGHT", JoinKind::Right),
    ] {
        let sql = format!(
            "SELECT a.Id FROM Samples a {syntax} JOIN Others b ON a.Id=b.Id IN 'remote' WHERE a.Id>0"
        );
        let result = parsed(&sql);
        assert_eq!(result.query().external.as_ref().unwrap().database, "remote");
        let TableSource::Join {
            kind: actual,
            condition,
            ..
        } = &result.query().from[0]
        else {
            panic!("joined source expected");
        };
        assert_eq!(*actual, kind);
        assert!(matches!(
            condition,
            Expression::Binary {
                operator: BinaryOperator::Equal,
                ..
            }
        ));
        assert!(result.query().selection.is_some());
        round_trip(&sql);
    }
}

#[test]
fn membership_and_database_in_can_follow_each_other_without_reinterpreting_values() {
    for condition in [
        "a.Id=b.Id AND a.Id IN (1,2)",
        "a.Id=b.Id AND a.Id NOT IN (1,2)",
        "a.Id IN (SELECT Id FROM Others)",
        "a.Id BETWEEN b.Id AND b.Id+2",
        "NOT a.Flag",
        "(a.Id=b.Id)",
    ] {
        let sql =
            format!("SELECT a.Id FROM Samples a INNER JOIN Others b ON {condition} IN 'remote'");
        let result = parsed(&sql);
        assert_eq!(result.query().external.as_ref().unwrap().database, "remote");
        round_trip(&sql);
    }
}

#[test]
fn external_connection_forms_and_comments_survive_join_boundaries() {
    for suffix in [
        "IN 'remote'",
        "IN /* database token */ \"remote\"",
        "IN 'remote' 'MS Access;'",
        "IN 'remote' [MS Access;]",
        "IN remote.accdb",
    ] {
        let sql = format!("SELECT a.Id FROM Samples a INNER JOIN Others b ON a.Id=b.Id {suffix}");
        let result = parsed(&sql);
        assert!(result.query().external.is_some());
        round_trip(&sql);
    }
}

#[test]
fn predicate_in_without_parentheses_stays_invalid_outside_from_boundary() {
    for sql in [
        "SELECT Id IN 'remote'",
        "SELECT Id IN 'remote' FROM Samples",
        "SELECT Id FROM Samples WHERE Id IN 'remote'",
        "SELECT Id FROM Samples WHERE Id NOT IN 'remote'",
        "SELECT Id FROM Samples INNER JOIN Others ON Samples.Id NOT IN 'remote'",
        "SELECT Id FROM Samples INNER JOIN Others ON Samples.Id IN ()",
        "SELECT Id FROM Samples INNER JOIN Others ON Samples.Id IN 'remote' IN 'second'",
    ] {
        assert!(parse_select(sql, ParseLimits::default()).is_err(), "{sql}");
    }
}
