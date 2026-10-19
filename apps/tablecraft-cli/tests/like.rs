mod support;
use support::run;
#[test]
fn cli_like_modes_lists_unicode_and_nulls_are_available_through_query_run() {
    for (mode, pattern) in [("ansi89", "a*"), ("ansi92", "a%")] {
        let sql = format!(
            "SELECT q.S FROM (SELECT 'ab' AS S UNION ALL SELECT 'b2' UNION ALL SELECT NULL) AS q WHERE q.S LIKE '{pattern}'"
        );
        let result = run(&[
            "query",
            "run",
            &sql,
            "--like-mode",
            mode,
            "--collation",
            "ordinal",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"S\n\"\"\"ab\"\"\"\n");
    }
    let result = run(&[
        "query",
        "run",
        "SELECT 'é🙂' LIKE '??' AS UnicodeMatch, NULL NOT LIKE '*' AS UnknownValue, 'b2' LIKE '[a-c]#' AS InRange",
        "--like-mode",
        "ansi89",
        "--collation",
        "ordinal",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"UnicodeMatch\tUnknownValue\tInRange\nTRUE\tNULL\tTRUE\n"
    );
}
#[test]
fn missing_policies_and_late_bad_patterns_publish_no_partial_output() {
    for args in [
        vec![
            "query",
            "run",
            "SELECT 'secret' LIKE '*'",
            "--collation",
            "ordinal",
        ],
        vec![
            "query",
            "run",
            "SELECT 'secret' LIKE '*'",
            "--like-mode",
            "ansi89",
        ],
        vec![
            "query",
            "run",
            "SELECT q.S LIKE q.P FROM (SELECT 'a' AS S, 'a*' AS P UNION ALL SELECT 'secret', '[') AS q",
            "--like-mode",
            "ansi89",
            "--collation",
            "ordinal",
        ],
        vec![
            "query",
            "run",
            "SELECT 'secret' LIKE '*'",
            "--like-mode",
            "unknown",
            "--collation",
            "ordinal",
        ],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        if !args.contains(&"unknown") {
            assert!(!String::from_utf8_lossy(&result.stderr).contains("secret"));
        }
    }
}
