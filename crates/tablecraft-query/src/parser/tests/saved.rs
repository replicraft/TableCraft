use super::queries::query_round_trip;
use super::*;
use crate::query::{Statement, definition::Definition};

#[test]
fn saved_view_and_procedure_forms_preserve_headers_and_bodies() {
    for sql in [
        "CREATE VIEW SampleView AS SELECT Code FROM Samples WHERE Code > 0",
        "CREATE VIEW SampleView (CodeOut, LabelOut) AS SELECT Code, Label FROM Samples",
        "CREATE PROCEDURE SampleQuery AS SELECT Code FROM Samples",
        "CREATE PROCEDURE SampleQuery [Minimum] LONG, [Pattern] TEXT(80) AS SELECT Code FROM Samples WHERE Code > [Minimum] AND Label LIKE [Pattern]",
        "CREATE PROCEDURE SampleQuery AS UPDATE Samples SET Code = Code + 1",
        "CREATE PROCEDURE SampleQuery AS CREATE TABLE ScratchRows (Code LONG)",
        "PROCEDURE SampleQuery; SELECT Code FROM Samples",
        "PROCEDURE SampleQuery [Minimum] LONG; SELECT Code FROM Samples WHERE Code > [Minimum]",
        "DROP VIEW SampleView",
        "DROP PROCEDURE SampleQuery",
    ] {
        query_round_trip(sql);
    }
    let parsed = query_round_trip(
        "PROCEDURE SampleQuery [Minimum] LONG; UPDATE Samples SET Code = [Minimum]",
    );
    let Statement::Definition(definition) = &parsed.query().statement else {
        panic!("definition expected")
    };
    let Definition::Procedure(procedure) = definition.as_ref() else {
        panic!("procedure expected")
    };
    assert!(!procedure.create);
    assert!(!procedure.as_separator);
    assert_eq!(procedure.parameters[0].name.as_str(), "Minimum");
    assert!(matches!(procedure.body.statement, Statement::Update(_)));
}

#[test]
fn saved_queries_reject_missing_multiple_and_nonretrieval_view_bodies() {
    for sql in [
        "CREATE VIEW SampleView AS SELECT Code INTO CopyRows FROM Samples",
        "CREATE VIEW SampleView AS UPDATE Samples SET Code = 0",
        "CREATE VIEW SampleView AS PARAMETERS p LONG; SELECT Code FROM Samples",
        "PARAMETERS p LONG; CREATE VIEW SampleView AS SELECT Code FROM Samples",
        "CREATE VIEW SampleView () AS SELECT Code FROM Samples",
        "CREATE VIEW SampleView AS",
        "CREATE PROCEDURE SampleQuery AS",
        "PROCEDURE SampleQuery;",
        "PROCEDURE SampleQuery SELECT Code FROM Samples",
        "CREATE PROCEDURE SampleQuery AS SELECT Code FROM Samples; DELETE FROM Samples",
        "CREATE PROCEDURE SampleQuery AS BEGIN SELECT Code FROM Samples END",
    ] {
        assert!(
            crate::parse_query(sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
    }
}

#[test]
fn procedure_nesting_and_parameter_width_are_bounded() {
    let sql = format!("{}SELECT 1", "CREATE PROCEDURE SampleQuery AS ".repeat(100));
    assert_eq!(
        crate::parse_query(&sql, ParseLimits::default())
            .unwrap_err()
            .kind,
        ParseErrorKind::Limit
    );
    for count in [255, 256] {
        let parameters = (0..count)
            .map(|i| format!("p{i} LONG"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("CREATE PROCEDURE SampleQuery {parameters} AS SELECT 1");
        let result = crate::parse_query(&sql, ParseLimits::default());
        if count == 255 {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err().kind, ParseErrorKind::Limit);
        }
    }
}
