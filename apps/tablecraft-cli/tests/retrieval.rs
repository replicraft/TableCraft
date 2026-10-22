mod support;
use support::run;
#[test]
fn run_executes_inline_rows_filtering_sorting_and_top_ties() {
    let result = run(&[
        "query",
        "run",
        "SELECT TOP 1 q.ID FROM (SELECT 1 AS ID, 4 AS Price UNION ALL SELECT 2, 8 UNION ALL SELECT 3, 8) AS q ORDER BY q.Price DESC",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    assert_eq!(result.stdout, b"ID\n2\n3\n");
}
#[test]
fn run_groups_inline_rows_and_returns_an_empty_header() {
    let result = run(&[
        "query",
        "run",
        "SELECT q.Category, Count(*) AS N, Sum(q.Price) AS Total FROM (SELECT 10 AS Category, 4 AS Price UNION ALL SELECT 10, 8 UNION ALL SELECT 20, 2) AS q GROUP BY q.Category HAVING Count(*) > 1 ORDER BY q.Category",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"Category\tN\tTotal\n10\t2\t12\n");
    let result = run(&[
        "query",
        "run",
        "SELECT q.ID FROM (SELECT 1 AS ID) AS q WHERE FALSE",
    ]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"ID\n");
}
#[test]
fn run_handles_joins_and_correlated_subqueries() {
    let result = run(&[
        "query",
        "run",
        "SELECT a.ID, b.ID AS [Match] FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS a LEFT JOIN (SELECT 2 AS ID) AS b ON a.ID=b.ID ORDER BY a.ID",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"ID\tMatch\n1\tNULL\n2\t2\n");
    let result = run(&[
        "query",
        "run",
        "SELECT q.ID FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS q WHERE EXISTS (SELECT 1 WHERE q.ID=2)",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"ID\n2\n");
}
#[test]
fn typed_parameters_and_explicit_text_collation_are_available() {
    let result = run(&[
        "query",
        "run",
        "PARAMETERS Floor INTEGER, Label TEXT(8); SELECT q.ID, Label AS Label FROM (SELECT 1 AS ID UNION ALL SELECT 3) AS q WHERE q.ID > Floor",
        "--param",
        "Floor=long:2",
        "--param",
        "Label=text:hello",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"ID\tLabel\n3\t\"\"\"hello\"\"\"\n");
    let result = run(&[
        "query",
        "run",
        "SELECT 'b' AS S UNION SELECT 'a' ORDER BY S",
        "--collation",
        "ordinal",
    ]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"S\n\"\"\"a\"\"\"\n\"\"\"b\"\"\"\n");
    let result = run(&[
        "query",
        "run",
        "PARAMETERS P LONG; SELECT P",
        "--param",
        "P=null:",
    ]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"P\nNULL\n");
}
#[test]
fn failures_return_exit_two_no_partial_output_and_no_sql_or_value_payloads() {
    for sql in [
        "SELECT 42 AS Visible, 1/0 AS Failure",
        "SELECT q.ID FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS q WHERE 1/(q.ID-2)>0",
        "SELECT * FROM PrivateDatabase",
        "SELECT SecretFunction('secret-payload')",
        "SELECT (SELECT 1 UNION ALL SELECT 2)",
        "SELECT 'secret-payload' AS S UNION SELECT 'other' ORDER BY S",
        "DELETE FROM PrivateDatabase",
    ] {
        let result = run(&["query", "run", sql]);
        assert_eq!(result.status.code(), Some(2), "{sql}");
        assert!(result.stdout.is_empty(), "{sql}");
        let error = String::from_utf8(result.stderr).unwrap();
        for secret in [
            "secret-payload",
            "PrivateDatabase",
            "SecretFunction",
            "Visible",
        ] {
            assert!(!error.contains(secret), "{error}");
        }
    }
    for param in [
        "P=long:secret-payload",
        "P=byte:256",
        "P=unknown:secret-payload",
        "P=null:bad",
        "=long:1",
    ] {
        let result = run(&[
            "query",
            "run",
            "PARAMETERS P LONG; SELECT P",
            "--param",
            param,
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("secret-payload"));
    }
}
#[test]
fn help_and_parameter_limits_describe_and_enforce_the_boundary() {
    let result = run(&["query", "run", "--help"]);
    assert!(result.status.success());
    let help = String::from_utf8(result.stdout).unwrap();
    assert!(help.contains("--param"));
    assert!(help.contains("--collation"));
    assert!(help.contains("--database"));
    assert!(help.contains("stable/offline copy"));
    let big = format!("P=text:{}", "x".repeat(65_536));
    let result = run(&[
        "query",
        "run",
        "PARAMETERS P TEXT; SELECT P",
        "--param",
        &big,
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn output_limit_failure_cannot_publish_earlier_rows() {
    let numbers = (1..=12)
        .map(|n| {
            if n == 1 {
                "SELECT 1 AS ID".to_string()
            } else {
                format!("SELECT {n}")
            }
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let sql = format!(
        "SELECT a.S FROM (SELECT '{}' AS S) AS a, ({numbers}) AS b, ({numbers}) AS c",
        "x".repeat(8192)
    );
    let result = run(&["query", "run", &sql]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("output limit exceeded"));
}

#[test]
fn currency_parameters_keep_exact_full_range_values_and_invariant_input() {
    for text in [
        "-922337203685477.5808",
        "922337203685477.5807",
        "900719925474.0993",
        "0.0001",
    ] {
        let binding = format!("P=currency:{text}");
        let result = run(&[
            "query",
            "run",
            "PARAMETERS P CURRENCY; SELECT P AS Amount",
            "--param",
            &binding,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            format!("Amount\n{text}\n")
        );
    }
    for text in [
        "922337203685477.5808",
        "-922337203685477.5809",
        "1,25",
        "$1",
        "NaN",
    ] {
        let binding = format!("P=currency:{text}");
        let result = run(&[
            "query",
            "run",
            "PARAMETERS P CURRENCY; SELECT P AS Amount",
            "--param",
            &binding,
        ]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn currency_parameter_rounding_reuses_the_exact_text_conversion_policy() {
    for (text, expected) in [
        ("0.00005", "0.0000"),
        ("0.00015", "0.0002"),
        ("-0.00015", "-0.0002"),
    ] {
        let binding = format!("P=currency:{text}");
        let result = run(&[
            "query",
            "run",
            "PARAMETERS P CURRENCY; SELECT P AS Amount",
            "--param",
            &binding,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            format!("Amount\n{expected}\n")
        );
    }
}
