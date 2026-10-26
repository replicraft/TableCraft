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

#[test]
fn implicit_like_string_conversion_matches_explicit_conversion_in_both_profiles() {
    for declared in [false, true] {
        let mut args = vec![
            "query",
            "run",
            "SELECT 12 LIKE '1*' AS NumericMatch, True LIKE 'T*' AS BooleanMatch, 12 LIKE 12 AS BothNumeric, #2000-01-01# LIKE '2000*' AS DateMatch, 12 NOT LIKE '2*' AS NegatedMatch",
            "--like-mode",
            "ansi89",
            "--collation",
            "ordinal",
            "--number-format",
            "invariant",
            "--date-format",
            "iso",
        ];
        if declared {
            args.push("--declared-arithmetic");
        }
        let result = run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"NumericMatch\tBooleanMatch\tBothNumeric\tDateMatch\tNegatedMatch\nTRUE\tTRUE\tTRUE\tTRUE\tTRUE\n"
        );
    }
}

#[test]
fn implicit_like_conversion_preserves_null_and_buffered_conversion_failures() {
    let result = run(&[
        "query",
        "run",
        "SELECT NULL LIKE 12 AS UnknownMatch",
        "--like-mode",
        "ansi89",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"UnknownMatch\nNULL\n");

    for sql in [
        "SELECT 12 LIKE '1*'",
        "SELECT #2000-01-01# LIKE '*'",
        "SELECT NULL LIKE (1/0)",
        "SELECT CVErr(7) LIKE '*'",
    ] {
        let result = run(&[
            "query",
            "run",
            sql,
            "--like-mode",
            "ansi89",
            "--collation",
            "ordinal",
        ]);
        assert_eq!(result.status.code(), Some(2), "{sql}");
        assert!(result.stdout.is_empty(), "{sql}");
    }
}
