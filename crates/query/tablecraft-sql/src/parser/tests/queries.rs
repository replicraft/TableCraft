//! Original corpus covering each owned query family. No Access execution evidence.
use super::*;
use crate::{
    parse_query,
    query::{ReadBody, Statement},
    select::{FunctionArgument, UnaryOperator},
};
const QUERIES: &[&str] = &[
    "SELECT Code, Label FROM Samples",
    "SELECT Date() AS Today, 1 + 2 AS Total",
    "[Saved Query A] UNION ALL [Saved Query B] ORDER BY Code",
    "SELECT DISTINCT TOP 5 PERCENT Code FROM Samples ORDER BY Code DESC",
    "SELECT DISTINCTROW TOP 3 * FROM Samples",
    "SELECT 1.20 + 2e3 AS Amount, IIf(Code > 1, 'yes', 'no') AS Verdict FROM Samples",
    "SELECT Sum(Code), Label FROM Samples GROUP BY Label HAVING Count(*) > 1 ORDER BY Sum(Code)",
    "SELECT s.Code, o.Label FROM Samples s INNER JOIN Others o ON s.Code = o.Code",
    "SELECT s.* FROM (Samples AS s LEFT JOIN Others AS o ON s.Code = o.Code) RIGHT JOIN Thirds AS t ON o.Code = t.Code",
    "SELECT s.Code FROM Samples AS s, Others AS o WHERE s.Code = o.Code",
    "SELECT s.Code FROM (SELECT Code FROM Samples WHERE Code > 0) AS s",
    "SELECT * FROM Samples WHERE Code IN (SELECT Code FROM Others)",
    "SELECT * FROM Samples WHERE Code NOT IN (SELECT Code FROM Others WHERE Flag = TRUE)",
    "SELECT * FROM Samples WHERE EXISTS (SELECT Code FROM Others WHERE Others.Code = Samples.Code)",
    "SELECT * FROM Samples WHERE NOT EXISTS (SELECT Code FROM Others)",
    "SELECT * FROM Samples WHERE Code > ALL (SELECT Code FROM Others)",
    "SELECT * FROM Samples WHERE Code = ANY (SELECT Code FROM Others)",
    "SELECT * FROM Samples WHERE Code <> SOME (SELECT Code FROM Others)",
    "SELECT (SELECT Max(Code) FROM Others) AS Largest FROM Samples",
    "SELECT Code FROM Samples UNION SELECT Code FROM Others UNION ALL TABLE Thirds ORDER BY Code",
    "TABLE Samples UNION ALL SELECT Code FROM Others",
    "PARAMETERS [Minimum score] Long, [When?] DateTime; SELECT * FROM Samples WHERE Code > [Minimum score] AND CreatedAt > [When?]",
    "PARAMETERS [Title] Text(80), [Rate] Decimal(28, 4); SELECT Nz([Title], '') FROM Samples",
    "PARAMETERS [Path] CHAR(40); UPDATE Samples SET Label = [Path] WHERE Code = 1",
    "TRANSFORM Sum(Code) SELECT Label FROM Samples GROUP BY Label PIVOT Flag",
    "TRANSFORM Count(*) SELECT Label FROM Samples WHERE Code > 0 GROUP BY Label ORDER BY Label PIVOT Flag IN (TRUE, FALSE)",
    "INSERT INTO Samples (Code, Label) VALUES (1, 'new')",
    "INSERT INTO Samples VALUES (1, 'new', NULL)",
    "INSERT INTO Samples (Code, Label) SELECT Code, Label FROM Others WHERE Code > 0",
    "INSERT INTO Samples (Code) IN 'destination.accdb' SELECT Code FROM Others",
    "UPDATE Samples SET Code = Code + 1, Label = Nz(Label, 'empty') WHERE Code BETWEEN 1 AND 5",
    "UPDATE Samples AS s INNER JOIN Others AS o ON s.Code = o.Code SET s.Label = o.Label WHERE o.Flag = TRUE",
    "DELETE FROM Samples WHERE Code IN (1, 2)",
    "DELETE * FROM Samples WHERE Code = 1",
    "DELETE s.* FROM Samples AS s INNER JOIN Others AS o ON s.Code = o.Code WHERE o.Flag = FALSE",
    "SELECT Code INTO NewSamples FROM Samples",
    "SELECT * FROM Samples IN OtherDB.mdb",
    "SELECT * FROM Samples IN '' [dBASE IV;DATABASE=C:\\warehouse;]",
    "SELECT * FROM Samples IN 'workbook.xls' 'Excel 8.0;'",
    "SELECT Code INTO NewSamples IN 'destination.accdb' FROM Samples",
    "SELECT * FROM Samples IN 'source.accdb' WITH OWNERACCESS OPTION",
    "SELECT * FROM Samples WHERE CreatedAt BETWEEN #1/1/2025# AND #12/31/2025 11:59:59 PM#",
    "SELECT [Forms]![Search Form].[Code] FROM Samples",
    "SELECT Mid(Label, , 3), Count(*), Date() FROM Samples",
    "SELECT -2 ^ 2, 2 ^ 3 ^ 2, 7 \\ 2, 7 Mod 3, 1 Xor 2 Eqv 3 Imp 4 FROM Samples",
    "SELECT &HFF, &O77, -9223372036854775808, - -9223372036854775808 FROM Samples",
    "SELECT 'Sam''s' & \" text\", Label + 'x' FROM Samples WHERE Label NOT LIKE 'a*'",
];
pub(super) fn query_round_trip(sql: &str) -> crate::ParsedQuery {
    let result = parse_query(sql, ParseLimits::default()).unwrap_or_else(|e| panic!("{sql}: {e}"));
    assert_eq!(result.source(), sql);
    let formatted = result.query().to_string();
    let decoded = parse_query(&formatted, ParseLimits::default())
        .unwrap_or_else(|e| panic!("{formatted}: {e}"));
    assert_eq!(result.query(), decoded.query(), "{formatted}");
    assert_eq!(decoded.query().to_string(), formatted);
    result
}
#[test]
fn all_query_families_round_trip_without_loss() {
    for sql in QUERIES {
        query_round_trip(sql);
    }
}
#[test]
fn parameters_and_union_modifiers_remain_owned_and_ordered() {
    let result = query_round_trip(
        "PARAMETERS [Limit] Long; SELECT Code FROM Samples UNION ALL SELECT Code FROM Others UNION TABLE Thirds ORDER BY Code DESC",
    );
    assert_eq!(result.query().parameters[0].name.as_str(), "Limit");
    assert_eq!(result.query().parameters[0].data_type.name, "LONG");
    let Statement::Read(query) = &result.query().statement else {
        panic!("read expected");
    };
    let ReadBody::Union { rest, .. } = &query.body else {
        panic!("union expected");
    };
    assert_eq!(rest.len(), 2);
    assert!(rest[0].all);
    assert!(!rest[1].all);
    assert!(query.order_by[0].descending);
}
#[test]
fn crosstab_headings_are_not_membership_syntax() {
    let result = query_round_trip(
        "TRANSFORM Sum(Code) SELECT Label FROM Samples GROUP BY Label PIVOT Flag IN ('a', 'b', 'a')",
    );
    let Statement::Crosstab(cross) = &result.query().statement else {
        panic!("crosstab expected");
    };
    assert!(matches!(cross.pivot, Expression::Field(_)));
    assert_eq!(
        cross.headings.as_ref().unwrap(),
        &[
            Expression::Text("a".into()),
            Expression::Text("b".into()),
            Expression::Text("a".into())
        ]
    );
}
#[test]
fn access_precedence_and_argument_omission_are_explicit() {
    let result = parsed("SELECT -2 ^ 2 AS Computed FROM Samples");
    let Projection::Expression {
        expression:
            Expression::Unary {
                operator: UnaryOperator::Minus,
                expression,
            },
        ..
    } = &result.query().projection[0]
    else {
        panic!("unary expected");
    };
    assert!(matches!(
        expression.as_ref(),
        Expression::Binary {
            operator: BinaryOperator::Power,
            ..
        }
    ));
    let result = parsed("SELECT 1 + 2 & 'x' AS Computed FROM Samples");
    let Projection::Expression {
        expression:
            Expression::Binary {
                left,
                operator: BinaryOperator::Concatenate,
                ..
            },
        ..
    } = &result.query().projection[0]
    else {
        panic!("concatenation expected");
    };
    assert!(matches!(
        left.as_ref(),
        Expression::Binary {
            operator: BinaryOperator::Add,
            ..
        }
    ));
    let result = parsed("SELECT Mid(Label, , 3) FROM Samples");
    let Projection::Expression {
        expression: Expression::Call { arguments, .. },
        ..
    } = &result.query().projection[0]
    else {
        panic!("call expected");
    };
    assert_eq!(arguments[1], FunctionArgument::Missing);
    assert_ne!(arguments[1], FunctionArgument::Expression(Expression::Null));
}
#[test]
fn date_and_numeric_spelling_are_not_coerced_to_values() {
    let result = parsed("SELECT #1/2/2025 3:04 PM#, 1.200, 1e9999 FROM Samples");
    for (index, expected) in [
        Expression::DateLiteral("1/2/2025 3:04 PM".into()),
        Expression::Number("1.200".into()),
        Expression::Number("1e9999".into()),
    ]
    .iter()
    .enumerate()
    {
        let Projection::Expression { expression, .. } = &result.query().projection[index] else {
            panic!("expression expected");
        };
        assert_eq!(expression, expected);
    }
}
#[test]
fn foreign_syntax_and_malformed_statements_fail_closed() {
    for sql in [
        "SELECT Sum(DISTINCT Code) FROM Samples",
        "SELECT Code FROM Samples LIMIT 5",
        "SELECT Code FROM Samples EXCEPT SELECT Code FROM Others",
        "SELECT * FROM Samples FULL OUTER JOIN Others ON Samples.Code = Others.Code",
        "SELECT Row_Number() OVER (ORDER BY Code) FROM Samples",
        "SELECT * FROM Samples WHERE Flag IS TRUE",
        "SELECT * FROM Samples WHERE Code == 1",
        "SELECT * FROM Samples; DELETE FROM Samples",
        "SELECT * FROM Samples;;",
        "PARAMETERS [Minimum score]; SELECT * FROM Samples",
        "PARAMETERS x MadeUpType; SELECT * FROM Samples",
        "PARAMETERS x LONG SELECT * FROM Samples",
        "PARAMETERS x TEXT(-1); SELECT * FROM Samples",
        "SELECT * FROM Samples UNION",
        "SELECT * FROM Samples UNION DISTINCT SELECT * FROM Others",
        "TRANSFORM Sum(Code) SELECT Label FROM Samples GROUP BY Label",
        "TRANSFORM Sum(Code) SELECT Label FROM Samples PIVOT Flag IN ()",
        "INSERT INTO Samples (Code) VALUES ()",
        "INSERT INTO Samples (Code) VALUES (1), (2)",
        "UPDATE Samples SET",
        "UPDATE Samples SET Code =",
        "DELETE FROM",
        "DELETE Label FROM Samples",
        "SELECT * FROM Samples WHERE Code IN ()",
        "SELECT Code FROM Samples WHERE Code BETWEEN 1 OR 2",
        "SELECT ## FROM Samples",
        "SELECT #unfinished FROM Samples",
    ] {
        assert!(
            parse_query(sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
    }
}
#[test]
fn recursion_is_bounded_across_subquery_expression_and_relation_boundaries() {
    let mut sql = "SELECT Code FROM Samples".to_owned();
    for _ in 0..33 {
        sql = format!("SELECT ({sql}) FROM Samples");
    }
    assert_eq!(
        parse_query(&sql, ParseLimits::default()).unwrap_err().kind,
        ParseErrorKind::Limit
    );
    let mut sql = "Code".to_owned();
    for _ in 0..20 {
        sql = format!("NOT NOT NOT NOT ({sql})");
    }
    assert_eq!(
        parse_query(
            &format!("SELECT {sql} FROM Samples"),
            ParseLimits::default()
        )
        .unwrap_err()
        .kind,
        ParseErrorKind::Limit
    );
    // A wide UNION stays iterative, rather than increasing the recursion depth.
    let sql = vec!["SELECT Code FROM Samples"; 40].join(" UNION ALL ");
    query_round_trip(&sql);
}

#[test]
fn make_table_destinations_stay_distinct_from_external_sources() {
    let sql = "SELECT Code INTO NewSamples IN 'destination.accdb' FROM Samples IN 'source.accdb'";
    let result = query_round_trip(sql);
    let Statement::MakeTable(select) = &result.query().statement else {
        panic!("make-table expected");
    };
    assert_eq!(
        select.into_external.as_ref().unwrap().database,
        "destination.accdb"
    );
    assert_eq!(select.external.as_ref().unwrap().database, "source.accdb");
    for sql in [
        "SELECT (SELECT Code INTO CopyRows FROM Samples) FROM Samples",
        "SELECT * FROM (SELECT Code INTO CopyRows FROM Samples) AS s",
        "SELECT Code INTO CopyRows FROM Samples UNION SELECT Code FROM Others",
        "SELECT Code FROM Others UNION SELECT Code INTO CopyRows FROM Samples",
        "INSERT INTO Samples SELECT Code INTO CopyRows FROM Others",
        "TRANSFORM Sum(Code) SELECT Label INTO CopyRows FROM Samples GROUP BY Label PIVOT Flag",
    ] {
        assert!(
            crate::parse_query(sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
    }
}

#[test]
fn guid_identity_and_qualified_function_syntax_round_trip() {
    for sql in [
        "SELECT @@IDENTITY",
        "SELECT VBA.Left$(Label, 2) FROM Samples",
        "SELECT [Custom Module].[Transform Name](Code, , NULL) FROM Samples",
        "SELECT * FROM Samples WHERE ReplicaID = {GUID {12345678-90AB-CDEF-1234-567890ABCDEF}}",
        "SELECT {guid {abcdef01-2345-6789-abcd-0123456789ab}} FROM Samples",
    ] {
        query_round_trip(sql);
    }
    let result = parsed("SELECT VBA.Left$(Label, 2) FROM Samples");
    let Projection::Expression {
        expression: Expression::Call {
            qualifier, name, ..
        },
        ..
    } = &result.query().projection[0]
    else {
        panic!("call expected");
    };
    assert_eq!(qualifier[0].as_str(), "VBA");
    assert_eq!(name.as_str(), "Left$");
    let result = parsed("SELECT {GUID {12345678-90AB-CDEF-1234-567890ABCDEF}} FROM Samples");
    assert!(
        matches!(&result.query().projection[0], Projection::Expression { expression: Expression::GuidLiteral(guid), .. } if guid == "12345678-90AB-CDEF-1234-567890ABCDEF")
    );
}
#[test]
fn invalid_function_and_guid_identifiers_are_not_normalized_into_valid_sql() {
    for sql in [
        "SELECT []() FROM Samples",
        "SELECT [bad.name]() FROM Samples",
        "SELECT [bad!name]() FROM Samples",
        "SELECT FROM(Code)",
        "SELECT DISTINCT ON (Code) Code FROM Samples",
        "SELECT {GUID {123}}",
        "SELECT {GUID {1234567890AB-CDEF-1234-567890ABCDEF}}",
        "SELECT {GUID {zzzzzzzz-90AB-CDEF-1234-567890ABCDEF}}",
        "SELECT {GUID {12345678-90AB-CDEF-1234-567890ABCDEF}",
        "SELECT @@MADEUP",
        "SELECT {d '2025-01-01'}",
        "SELECT F(Code => 1) FROM Samples",
    ] {
        assert!(
            crate::parse_query(sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
    }
}

#[test]
fn crosstab_subqueries_are_allowed_in_predicates_but_not_output_expressions() {
    query_round_trip(
        "TRANSFORM Sum(Code) SELECT Label FROM Samples WHERE Code IN (SELECT Code FROM Others) GROUP BY Label PIVOT Flag",
    );
    for output in [
        "(SELECT Max(Code) FROM Others)",
        "Nz((SELECT Max(Code) FROM Others), 0)",
        "Code + (SELECT Max(Code) FROM Others)",
        "Code BETWEEN 0 AND (SELECT Max(Code) FROM Others)",
    ] {
        let sql =
            format!("TRANSFORM Sum(Code) SELECT {output} FROM Samples GROUP BY Label PIVOT Flag");
        assert!(
            parse_query(&sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
    }
}
