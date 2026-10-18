//! Original syntax cases based on Microsoft references in docs/SQL_PARSER.md.
//! These checks do not establish Access comparison, NULL or collation behavior.

use super::*;

#[test]
fn retains_written_bound_order_and_not_modifier() {
    for (first, second) in [(5, 1), (1, 5), (1, 1)] {
        for negated in [false, true] {
            let modifier = if negated { "NOT " } else { "" };
            let sql =
                format!("SELECT * FROM Samples WHERE Code {modifier}BETWEEN {first} AND {second}");
            let result = parsed(&sql);
            let Expression::Between {
                expression,
                first_bound,
                second_bound,
                negated: actual,
            } = result.query().selection.as_ref().unwrap()
            else {
                panic!("BETWEEN expected");
            };
            assert!(matches!(expression.as_ref(), Expression::Field(_)));
            assert_eq!(first_bound.as_ref(), &Expression::Integer(first));
            assert_eq!(second_bound.as_ref(), &Expression::Integer(second));
            assert_eq!(*actual, negated);
            assert_eq!(result.source(), sql);
            assert_eq!(
                result.query().to_string(),
                format!(
                    "SELECT * FROM [Samples] WHERE ([Code] {modifier}BETWEEN {first} AND {second});"
                )
            );
            round_trip(&sql);
        }
    }
    assert_ne!(
        parsed("SELECT * FROM Samples WHERE Code BETWEEN 5 AND 1").query(),
        parsed("SELECT * FROM Samples WHERE Code >= 5 AND Code <= 1").query()
    );
}

#[test]
fn delimiter_and_does_not_swallow_logical_and_or() {
    let result =
        parsed("SELECT * FROM Samples WHERE Code BETWEEN 5 AND 1 AND Flag = TRUE OR Label IS NULL");
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
    assert!(matches!(
        left.as_ref(),
        Expression::Between { negated: false, .. }
    ));
    for sql in [
        "SELECT * FROM Samples WHERE Code BETWEEN 5 AND 1 AND Flag = TRUE OR Label IS NULL",
        "SELECT * FROM Samples WHERE Flag = TRUE OR Code NOT BETWEEN 5 AND 1 AND Label IS NULL",
        "SELECT * FROM Samples WHERE Code BETWEEN (StartCode = 1) AND (EndCode = 2)",
        "SELECT * FROM Samples WHERE (Code BETWEEN 5 AND 1) BETWEEN FALSE AND TRUE",
    ] {
        round_trip(sql);
    }
}

#[test]
fn every_between_operand_obeys_expression_depth_limits() {
    for sql in [
        "SELECT * FROM Samples WHERE (Code = 1) BETWEEN 0 AND 1",
        "SELECT * FROM Samples WHERE Code BETWEEN (StartCode = 1) AND 2",
        "SELECT * FROM Samples WHERE Code BETWEEN 1 AND (EndCode = 2)",
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
        let error = parse_select(
            sql,
            ParseLimits {
                max_depth: 2,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.kind, ParseErrorKind::Limit);
    }
    let sql = "SELECT * FROM Samples WHERE Code BETWEEN 5 AND 1";
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
}

#[test]
fn malformed_bounds_and_unsupported_operands_fail_closed() {
    for sql in [
        "SELECT * FROM Samples WHERE Code BETWEEN",
        "SELECT * FROM Samples WHERE Code BETWEEN 1",
        "SELECT * FROM Samples WHERE Code BETWEEN 1 OR 5",
        "SELECT * FROM Samples WHERE Code BETWEEN AND 5",
        "SELECT * FROM Samples WHERE Code BETWEEN 1 AND",
        "SELECT * FROM Samples WHERE Code BETWEEN 1 AND 5 AND",
        "SELECT * FROM Samples WHERE Code BETWEEN SYMMETRIC 5 AND 1",
        "SELECT * FROM Samples WHERE Code BETWEEN 1.5 AND 5",
        "SELECT * FROM Samples WHERE Code BETWEEN 1 AND Nz(EndCode, 5)",
        "SELECT * FROM Samples WHERE Code BETWEEN 1 AND (SELECT Code FROM Others)",
        "SELECT * FROM Samples WHERE Code BETWEEN #1/1/2025# AND #2/1/2025#",
    ] {
        assert!(
            parse_select(sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
    }
}
