mod between;
mod corpus;
mod definition;
mod external_joins;
mod generated;
mod identifiers;
mod in_list;
mod limits;
mod parameter_prompts;
mod queries;
mod saved;
mod terminators;
mod text;

use super::*;
use crate::select::{BinaryOperator, Expression, Projection, SelectPredicate};

fn parsed(sql: &str) -> ParsedSelect {
    parse_select(sql, ParseLimits::default()).unwrap_or_else(|error| panic!("{sql}: {error}"))
}

fn round_trip(sql: &str) {
    let first = parsed(sql);
    let normalized = first.query().to_string();
    let second = parsed(&normalized);
    assert_eq!(first.query(), second.query(), "{normalized}");
    assert_eq!(second.query().to_string(), normalized);
}

#[test]
fn preserves_source_and_owned_structure() {
    let sql = "SELECT TOP 7 PERCENT s.[Display Name] AS [Shown], s.*\nFROM [Sample Rows] AS s WHERE s.Score >= -42 ORDER BY s.Score DESC;";
    let result = parsed(sql);
    assert_eq!(result.source(), sql);
    let query = result.query();
    assert_eq!(
        query.predicate,
        SelectPredicate::Top {
            count: 7,
            percent: true
        }
    );
    let crate::select::TableSource::Table(table) = &query.from[0] else {
        panic!("table expected");
    };
    assert_eq!(table.name.as_str(), "Sample Rows");
    assert_eq!(table.alias.as_ref().unwrap().as_str(), "s");
    let Projection::Field { field, alias } = &query.projection[0] else {
        panic!("field expected");
    };
    assert_eq!(field.name.as_str(), "Display Name");
    assert_eq!(field.qualifier.as_ref().unwrap().as_str(), "s");
    assert_eq!(alias.as_ref().unwrap().as_str(), "Shown");
    assert_eq!(
        query.projection[1],
        Projection::Wildcard(table.alias.clone())
    );
    let Expression::Binary {
        operator, right, ..
    } = query.selection.as_ref().unwrap()
    else {
        panic!("comparison expected");
    };
    assert_eq!(*operator, BinaryOperator::GreaterOrEqual);
    assert_eq!(**right, Expression::Integer(-42));
    assert!(query.order_by[0].descending);
    round_trip(sql);
}

#[test]
fn preserves_access_predicates_without_equating_them() {
    let distinct = parsed("SELECT DISTINCT Label FROM Samples");
    let distinctrow = parsed("SELECT DISTINCTROW Label FROM Samples");
    assert_eq!(distinct.query().predicate, SelectPredicate::Distinct);
    assert_eq!(distinctrow.query().predicate, SelectPredicate::DistinctRow);
    assert_ne!(distinct.query(), distinctrow.query());
    assert_eq!(
        parsed("SELECT ALL * FROM Samples").query(),
        parsed("SELECT * FROM Samples").query()
    );
}

#[test]
fn logical_precedence_and_parentheses_are_preserved() {
    let result = parsed("SELECT * FROM Samples WHERE a = 1 OR b = 2 AND c = 3");
    let Expression::Binary {
        operator, right, ..
    } = result.query().selection.as_ref().unwrap()
    else {
        panic!("OR expected");
    };
    assert_eq!(*operator, BinaryOperator::Or);
    assert!(matches!(
        **right,
        Expression::Binary {
            operator: BinaryOperator::And,
            ..
        }
    ));
    round_trip("SELECT * FROM Samples WHERE (a = 1 OR b = 2) AND c = 3");
}

#[test]
fn diagnostics_report_location_without_sql_payloads() {
    let error = parse_select(
        "SELECT *\nFROM Samples\nJOIN PrivateName ON secret = 'private'",
        ParseLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::Unsupported);
    assert_eq!(error.location, SourceLocation { line: 3, column: 1 });
    assert!(!format!("{error:?} {error}").contains("private"));
    let error = parse_select(
        "SELECT * FROM Samples WHERE note = 'unterminated",
        ParseLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::Syntax);
    assert!(error.location.column > 1);
}

#[test]
fn whitespace_comments_and_literal_delimiters_do_not_change_structure() {
    let source = " SELECT /* ( # */ * FROM Samples -- )\n WHERE note = 'a''b (#)' ; ";
    assert_eq!(parsed(source).source(), source);
    round_trip(source);
    round_trip("SELECT [Étiquette] FROM [Données] WHERE [Étiquette] = 'こんにちは'");
    round_trip("SELECT * FROM Samples WHERE Label = '!= =='");
    let error = parse_select(
        "SELECT * FROM Samples\r\nWHERE [Étiquette] != 1",
        ParseLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::Unsupported);
    assert_eq!(
        error.location,
        SourceLocation {
            line: 2,
            column: 19
        }
    );
}
