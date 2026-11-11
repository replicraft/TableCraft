//! Independent ACE declaration syntax; these tests do not execute provider DDL.
mod normalization;

use super::{queries::query_round_trip, *};
use crate::{
    query::{Statement, definition::*},
    select::{Expression, FieldReference, Name},
};

fn table(sql: &str) -> CreateTable {
    let Statement::Definition(definition) = query_round_trip(sql).into_query().statement else {
        panic!("expected DDL");
    };
    let Definition::CreateTable(table) = *definition else {
        panic!("expected CREATE TABLE")
    };
    table
}

#[test]
fn documented_defaults_checks_and_altered_column_annotations_round_trip() {
    for sql in [
        "CREATE TABLE Samples (Code LONG PRIMARY KEY, Address TEXT(40) DEFAULT Unknown)",
        "CREATE TABLE Samples (Code LONG, Amount CURRENCY DEFAULT 0.0000, CONSTRAINT PositiveAmount CHECK (Amount > 0))",
        "ALTER TABLE Samples ADD CONSTRAINT LimitRule CHECK (Amount <= (SELECT Max(CreditLimit) FROM Limits WHERE Limits.Code=Samples.Code))",
        "ALTER TABLE Samples ALTER COLUMN Code LONG CONSTRAINT SampleKey PRIMARY KEY",
        "ALTER TABLE Samples ALTER COLUMN Label TEXT(40) CONSTRAINT RequiredLabel NOT NULL",
        "ALTER TABLE Samples ALTER COLUMN Label TEXT(40) DEFAULT Unknown",
        "ALTER TABLE Samples ALTER COLUMN Label TEXT(40) DEFAULT 'Unknown' NOT NULL",
        "ALTER TABLE Samples ADD COLUMN Active BIT DEFAULT False NOT NULL",
        "CREATE TABLE Samples (Label TEXT(40) WITH COMPRESSION DEFAULT 'a, ) b' CONSTRAINT LabelRule CHECK (Len(Label)<=40))",
        "CREATE TABLE Samples (Amount CURRENCY CHECK (Amount BETWEEN -100 AND 100))",
        "ALTER TABLE Samples ADD COLUMN Total LONG DEFAULT 1+2 NOT NULL CONSTRAINT PositiveTotal CHECK (Total > 0)",
        "ALTER TABLE Samples ALTER COLUMN Recorded DATETIME DEFAULT DateSerial(2000,1,1)",
    ] {
        query_round_trip(sql);
    }
}

#[test]
fn bare_default_word_bracketed_reference_quoted_text_and_literals_stay_distinct() {
    let created = table(
        "CREATE TABLE Samples (A TEXT DEFAULT UnKnOwN, B TEXT DEFAULT [Unknown], C TEXT DEFAULT 'Unknown', D BIT DEFAULT False, E LONG DEFAULT NULL, F DOUBLE DEFAULT 1.2300e2)",
    );
    let columns = created
        .elements
        .into_iter()
        .map(|element| {
            let TableElement::Column(column) = element else {
                panic!("expected column")
            };
            column
        })
        .collect::<Vec<_>>();
    let Some(ColumnDefault::BareWord(word)) = &columns[0].default_value else {
        panic!("bare word")
    };
    assert_eq!(word.as_str(), "UnKnOwN");
    assert_eq!(
        columns[0].default_value.as_ref().unwrap().to_string(),
        "UnKnOwN"
    );
    let expression = |index: usize| match columns[index].default_value.as_ref().unwrap() {
        ColumnDefault::Expression(value) => value.as_ref(),
        ColumnDefault::BareWord(_) => panic!("expected expression"),
    };
    assert!(
        matches!(expression(1), Expression::Field(field) if field.qualifier.is_none() && field.name.as_str()=="Unknown")
    );
    assert_eq!(expression(2), &Expression::Text("Unknown".into()));
    assert_eq!(expression(3), &Expression::Boolean(false));
    assert_eq!(expression(4), &Expression::Null);
    assert_eq!(expression(5), &Expression::Number("1.2300e2".into()));
}

#[test]
fn alter_column_retains_default_nullability_and_named_constraint_together() {
    let parsed = query_round_trip(
        "ALTER TABLE Samples ALTER COLUMN Amount DECIMAL(28,4) DEFAULT -1.2345 NOT NULL CONSTRAINT AmountRequired NOT NULL",
    );
    let Statement::Definition(definition) = &parsed.query().statement else {
        panic!("DDL")
    };
    let Definition::AlterTable(altered) = definition.as_ref() else {
        panic!("ALTER TABLE")
    };
    let AlterOperation::AlterColumn(column) = &altered.operation else {
        panic!("ALTER COLUMN")
    };
    assert_eq!(column.name.as_str(), "Amount");
    assert_eq!(column.data_type.name, "DECIMAL");
    assert_eq!(column.data_type.arguments, [28, 4]);
    assert!(column.not_null);
    assert!(column.default_value.is_some());
    let constraint = column.constraint.as_ref().unwrap();
    assert_eq!(constraint.name.as_ref().unwrap().as_str(), "AmountRequired");
    assert_eq!(constraint.kind, ConstraintKind::NotNull(Vec::new()));
}

#[test]
fn check_retains_a_real_nested_read_expression_in_the_owned_ast() {
    let created = table(
        "CREATE TABLE Samples (Amount CURRENCY, CONSTRAINT CreditRule CHECK (Amount <= (SELECT Max(CreditLimit) FROM Limits)))",
    );
    let TableElement::Constraint(constraint) = &created.elements[1] else {
        panic!("constraint")
    };
    assert_eq!(constraint.name.as_ref().unwrap().as_str(), "CreditRule");
    let ConstraintKind::Check(Expression::Binary { left, right, .. }) = &constraint.kind else {
        panic!("CHECK comparison")
    };
    assert!(matches!(left.as_ref(), Expression::Field(field) if field.name.as_str()=="Amount"));
    assert!(matches!(right.as_ref(), Expression::Subquery(_)));
}

#[test]
fn malformed_duplicate_or_escaping_annotations_fail_without_consuming_extra_sql() {
    for sql in [
        "CREATE TABLE Samples (Amount LONG DEFAULT)",
        "ALTER TABLE Samples ALTER COLUMN Amount LONG DEFAULT 1 DEFAULT 2",
        "ALTER TABLE Samples ALTER COLUMN Amount LONG NOT NULL NOT NULL",
        "ALTER TABLE Samples ALTER COLUMN Amount LONG PRIMARY KEY UNIQUE",
        "ALTER TABLE Samples ADD CONSTRAINT AmountRule CHECK ()",
        "ALTER TABLE Samples ADD CONSTRAINT AmountRule CHECK Amount > 0",
        "ALTER TABLE Samples ADD CONSTRAINT AmountRule CHECK ((Amount > 0)",
        "ALTER TABLE Samples ADD CONSTRAINT AmountRule CHECK ((DELETE FROM Samples))",
        "ALTER TABLE Samples ADD CONSTRAINT AmountRule CHECK ((SELECT Amount INTO Copy FROM Samples))",
        "CREATE TABLE Samples (Amount LONG DEFAULT 1); DROP TABLE Samples",
        "CREATE TABLE Samples (Amount LONG CHECK (Amount>0)); DELETE FROM Samples",
        "CREATE TABLE Samples (Amount LONG DEFAULT (1 NOT NULL))",
    ] {
        assert!(parse_query(sql, ParseLimits::default()).is_err(), "{sql}");
    }
    // The DEFAULT-only delimiter must not turn invalid ordinary expressions
    // into accepted postfix NOT NULL syntax.
    assert!(parse_query("SELECT Code NOT NULL FROM Samples", ParseLimits::default()).is_err());
    assert!(parse_expression_strict("Code NOT NULL", ParseLimits::default()).is_err());
}

#[test]
fn declaration_expressions_share_original_input_token_depth_and_recursion_guards() {
    let sql = "ALTER TABLE Samples ADD CONSTRAINT AmountRule CHECK (Amount > (SELECT Max(CreditLimit) FROM Limits))";
    for limits in [
        ParseLimits {
            max_bytes: sql.len() - 1,
            ..Default::default()
        },
        ParseLimits {
            max_tokens: 4,
            ..Default::default()
        },
        ParseLimits {
            max_depth: 1,
            ..Default::default()
        },
    ] {
        assert_eq!(
            parse_query(sql, limits).unwrap_err().kind,
            ParseErrorKind::Limit
        );
    }
    let folded = table(&format!(
        "CREATE TABLE Samples (Amount LONG DEFAULT {}1)",
        "- ".repeat(40)
    ));
    let TableElement::Column(column) = &folded.elements[0] else {
        panic!("column")
    };
    assert!(
        matches!(&column.default_value, Some(ColumnDefault::Expression(value)) if value.as_ref() == &Expression::Integer(1))
    );
    for sql in [
        format!(
            "CREATE TABLE Samples (Amount LONG DEFAULT {}1)",
            "- ".repeat(150)
        ),
        format!(
            "CREATE TABLE Samples (Active BIT DEFAULT {}True)",
            "NOT ".repeat(40)
        ),
        format!(
            "ALTER TABLE Samples ADD CONSTRAINT AmountRule CHECK ({}Amount>0)",
            "NOT ".repeat(40)
        ),
    ] {
        assert_eq!(
            parse_query(&sql, ParseLimits::default()).unwrap_err().kind,
            ParseErrorKind::Limit
        );
    }
}

#[test]
fn constructed_bare_default_words_cannot_inject_or_change_expression_class() {
    for value in [
        "Unknown; DROP TABLE Victim",
        "Unknown -- comment",
        "Unknown/**/",
        "Unknown Other",
        " Unknown",
        "Unknown ",
        "[Unknown]",
        "'Unknown'",
        "a.b",
        "a!b",
        "1",
        "NULL",
        "TRUE",
        "False",
        "Now()",
        "a+1",
    ] {
        assert!(
            BareDefaultWord::new(value, ParseLimits::default()).is_err(),
            "{value}"
        );
    }
    assert!(parse_query("SELECT Unknown FROM Samples", ParseLimits::default()).is_err());
    let word = BareDefaultWord::new("UnKnOwN", ParseLimits::default()).unwrap();
    assert_eq!(ColumnDefault::BareWord(word).to_string(), "UnKnOwN");
    for limits in [
        ParseLimits {
            max_bytes: 6,
            ..Default::default()
        },
        ParseLimits {
            max_tokens: 0,
            ..Default::default()
        },
        ParseLimits {
            max_depth: 0,
            ..Default::default()
        },
    ] {
        assert_eq!(
            BareDefaultWord::new("Unknown", limits).unwrap_err().kind,
            ParseErrorKind::Limit
        );
    }
    let name = Name::from_contents("Unknown; DROP TABLE Victim", ParseLimits::default()).unwrap();
    assert!(BareDefaultWord::new(name.as_str(), ParseLimits::default()).is_err());
    let expression = ColumnDefault::Expression(Box::new(Expression::Field(FieldReference {
        qualifier: None,
        name,
    })));
    assert_eq!(expression.to_string(), "[Unknown; DROP TABLE Victim]");
    let sql = format!("CREATE TABLE Samples (Label TEXT DEFAULT {expression})");
    query_round_trip(&sql);
}

#[test]
fn default_and_check_owned_expression_depth_has_exact_default_and_lower_boundaries() {
    for limit in [4, 32] {
        let at_boundary = format!("{}True", "NOT ".repeat(limit - 1));
        let above_boundary = format!("{}True", "NOT ".repeat(limit));
        for (prefix, suffix) in [
            ("CREATE TABLE Samples (Active BIT DEFAULT ", ")"),
            ("ALTER TABLE Samples ALTER COLUMN Active BIT DEFAULT ", ""),
            ("ALTER TABLE Samples ADD CONSTRAINT ActiveRule CHECK (", ")"),
        ] {
            let limits = ParseLimits {
                max_depth: limit,
                ..Default::default()
            };
            let accepted = format!("{prefix}{at_boundary}{suffix}");
            assert!(
                parse_query(&accepted, limits).is_ok(),
                "depth {limit}: {accepted}"
            );
            let refused = format!("{prefix}{above_boundary}{suffix}");
            assert_eq!(
                parse_query(&refused, limits).unwrap_err().kind,
                ParseErrorKind::Limit,
                "depth {limit}: {refused}"
            );
        }
    }
}
