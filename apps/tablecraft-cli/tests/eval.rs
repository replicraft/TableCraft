mod support;
use support::run;

#[test]
fn scalar_select_calculates_a_result_row_without_a_database() {
    let output = run(&[
        "query",
        "eval",
        "SELECT 2 + 3 * 4 AS Total, 'Hello' & ' world' AS Greeting, FALSE AND NULL AS Flag",
    ]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Total\tGreeting\tFlag\n14\t\"Hello world\"\tFALSE\n"
    );
    let output = run(&[
        "query",
        "eval",
        "SELECT ALL 10 / 4, NULL = NULL, 'a' + NULL, ''",
    ]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Expr1\tExpr2\tExpr3\tExpr4\n2.5\tNULL\tNULL\t\"\"\n"
    );
}

#[test]
fn text_results_are_quoted_and_controls_do_not_create_rows_or_terminal_sequences() {
    let output = run(&[
        "query",
        "eval",
        "SELECT 'a\tb\nc\r\"d\\e\u{001b}' AS [Text Column], 'NULL' AS NullText",
    ]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Text Column\tNullText\n\"a\\tb\\nc\\r\\\"d\\\\e\\u{001b}\"\t\"NULL\"\n"
    );
}

#[test]
fn query_execution_clauses_and_action_statements_are_rejected_without_output() {
    for sql in [
        "SELECT 1 FROM Samples",
        "SELECT 1 WHERE TRUE",
        "SELECT DISTINCT 1",
        "SELECT DISTINCTROW 1",
        "SELECT TOP 1 1",
        "SELECT 1 ORDER BY 1",
        "SELECT 1 GROUP BY 1",
        "SELECT 1 HAVING TRUE",
        "SELECT 1 WITH OWNERACCESS OPTION",
        "SELECT Code INTO CopyRows FROM Samples",
        "SELECT 1 FROM Samples IN 'other.accdb'",
        "SELECT 1 UNION SELECT 2",
        "TABLE Samples",
        "PARAMETERS p LONG; SELECT 1",
        "TRANSFORM Count(*) SELECT Label FROM Samples GROUP BY Label PIVOT Flag",
        "UPDATE Samples SET Code = 1",
        "DELETE FROM Samples",
        "INSERT INTO Samples VALUES (1)",
        "CREATE TABLE Samples (Code LONG)",
        "CREATE VIEW SampleView AS SELECT 1",
        "DROP TABLE Samples",
        "PROCEDURE SampleQuery; SELECT 1",
    ] {
        let output = run(&["query", "eval", sql]);
        assert_eq!(output.status.code(), Some(2), "{sql}");
        assert!(output.stdout.is_empty(), "{sql}");
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("source-free SELECT"),
            "{sql}"
        );
    }
}

#[test]
fn unsupported_and_failed_expressions_never_print_partial_success_or_payloads() {
    for (sql, message) in [
        (
            "SELECT 1 AS FirstValue, 1 / 0 AS SecondValue",
            "column 2: division by zero",
        ),
        ("SELECT 9223372036854775807 + 1", "numeric overflow"),
        ("SELECT MissingField", "field has no binding"),
        (
            "SELECT IIf(TRUE, 1, 0)",
            "expression form is not implemented",
        ),
        ("SELECT 'private payload' + 3", "type mismatch"),
        ("SELECT 'a' = 'a'", "requires a collation"),
        ("SELECT FALSE AND (1 / 0 = 0)", "division by zero"),
    ] {
        let output = run(&["query", "eval", sql]);
        assert_eq!(output.status.code(), Some(2), "{sql}");
        assert!(output.stdout.is_empty(), "{sql}");
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains(message), "{error}");
        assert!(!error.contains("private payload"));
    }
}

#[test]
fn parse_and_evaluation_limits_apply_before_any_success_output() {
    let large_text = "x".repeat(20_000);
    let cumulative =
        format!("SELECT 1 AS FirstValue, '{large_text}' & '{large_text}' AS SecondValue");
    let oversized = "x".repeat(65_537);
    for sql in [
        cumulative.as_str(),
        oversized.as_str(),
        "SELECT 1; SELECT 2",
    ] {
        let output = run(&["query", "eval", sql]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .starts_with("error:")
        );
    }
}

#[test]
fn eval_help_explains_the_supported_boundary() {
    let output = run(&["query", "eval", "--help"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("source-free SELECT"));
    assert!(text.contains("parameters and action statements are not supported"));
}
