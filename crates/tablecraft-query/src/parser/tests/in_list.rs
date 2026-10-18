//! Original list-predicate cases; primary source mapping is in docs/SQL_PARSER.md.

use super::*;

#[test]
fn retains_list_order_duplicates_null_and_negation() {
    for negated in [false, true] {
        let modifier = if negated { "NOT " } else { "" };
        let sql = format!("SELECT * FROM Samples WHERE Code {modifier}IN (5, 1, 5, NULL)");
        let result = parsed(&sql);
        let Expression::InList {
            expression,
            values,
            negated: actual,
        } = result.query().selection.as_ref().unwrap()
        else {
            panic!("IN expected");
        };
        assert!(matches!(expression.as_ref(), Expression::Field(_)));
        assert_eq!(
            values,
            &[
                Expression::Integer(5),
                Expression::Integer(1),
                Expression::Integer(5),
                Expression::Null
            ]
        );
        assert_eq!(*actual, negated);
        assert_eq!(result.source(), sql);
        assert_eq!(
            result.query().to_string(),
            format!("SELECT * FROM [Samples] WHERE ([Code] {modifier}IN (5, 1, 5, NULL));")
        );
        round_trip(&sql);
    }
}

#[test]
fn list_entries_remain_expressions_with_distinct_boundaries() {
    let result = parsed(r#"SELECT * FROM Samples WHERE Label IN ('a,b', "x", '', NULL)"#);
    let Expression::InList { values, .. } = result.query().selection.as_ref().unwrap() else {
        panic!("IN expected");
    };
    assert_eq!(
        values,
        &[
            Expression::Text("a,b".to_owned()),
            Expression::Text("x".to_owned()),
            Expression::Text(String::new()),
            Expression::Null
        ]
    );
    for sql in [
        "SELECT * FROM Samples WHERE Code IN (1)",
        "SELECT * FROM Samples WHERE Code IN (1, 2) AND Flag = TRUE OR Label IS NULL",
        "SELECT * FROM Samples WHERE Flag = TRUE OR Code NOT IN (1, 2) AND Label IS NULL",
        "SELECT * FROM Samples WHERE Code IN (StartCode, EndCode)",
        "SELECT * FROM Samples WHERE Code IN ((StartCode = 1), (EndCode BETWEEN 5 AND 1))",
        "SELECT * FROM Samples WHERE (Code IN (1, 2)) IN (FALSE, TRUE)",
        "SELECT * FROM Samples WHERE NULL NOT IN (1, NULL)",
    ] {
        round_trip(sql);
    }
    let result =
        parsed("SELECT * FROM Samples WHERE Code IN (1, 2) AND Flag = TRUE OR Label IS NULL");
    let Expression::Binary {
        left,
        operator: BinaryOperator::Or,
        ..
    } = result.query().selection.as_ref().unwrap()
    else {
        panic!("OR expected");
    };
    let Expression::Binary {
        left,
        operator: BinaryOperator::And,
        ..
    } = left.as_ref()
    else {
        panic!("AND expected");
    };
    assert!(matches!(left.as_ref(), Expression::InList { .. }));
}

#[test]
fn list_width_and_every_operand_obey_limits() {
    for sql in [
        "SELECT * FROM Samples WHERE (Code = 1) IN (0, 1)",
        "SELECT * FROM Samples WHERE Code IN ((StartCode = 1), 2)",
        "SELECT * FROM Samples WHERE Code IN (1, (EndCode = 2))",
    ] {
        assert!(
            parse_select(
                sql,
                ParseLimits {
                    max_depth: 3,
                    ..Default::default()
                }
            )
            .is_ok()
        );
        assert_eq!(
            parse_select(
                sql,
                ParseLimits {
                    max_depth: 2,
                    ..Default::default()
                }
            )
            .unwrap_err()
            .kind,
            ParseErrorKind::Limit
        );
    }
    let sql = "SELECT * FROM Samples WHERE Code IN (1, 2, 3, 4, 5)";
    assert!(
        parse_select(
            sql,
            ParseLimits {
                max_depth: 2,
                ..Default::default()
            }
        )
        .is_ok()
    );
    assert_eq!(
        parse_select(
            sql,
            ParseLimits {
                max_depth: 1,
                ..Default::default()
            }
        )
        .unwrap_err()
        .kind,
        ParseErrorKind::Limit
    );
    let wide = vec!["1"; 1_025].join(",");
    assert_eq!(
        parse_select(
            &format!("SELECT * FROM Samples WHERE Code IN ({wide})"),
            ParseLimits::default()
        )
        .unwrap_err()
        .kind,
        ParseErrorKind::Limit
    );
}

#[test]
fn empty_malformed_and_unsupported_lists_fail_closed() {
    for sql in [
        "SELECT * FROM Samples WHERE Code IN ()",
        "SELECT * FROM Samples WHERE Code NOT IN ()",
        "SELECT * FROM Samples WHERE Code IN (1,)",
        "SELECT * FROM Samples WHERE Code IN (,1)",
        "SELECT * FROM Samples WHERE Code IN (1,,2)",
        "SELECT * FROM Samples WHERE Code IN (1 2)",
        "SELECT * FROM Samples WHERE Code IN 1",
        "SELECT * FROM Samples WHERE Code IN (1",
        "SELECT * FROM Samples WHERE Code IN UNNEST(items)",
    ] {
        assert!(
            parse_select(sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
    }
}
