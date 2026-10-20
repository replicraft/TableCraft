mod support;
use support::run;

#[test]
fn source_free_predicates_preserve_reversed_bounds_exact_int64_and_null() {
    let result = run(&[
        "query",
        "eval",
        "SELECT 2 BETWEEN 3 AND 1 AS InRange, 9007199254740993 IN (9007199254740992,9007199254740993) AS Exact, 2 NOT IN (1,Null) AS [Unknown]",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"InRange\tExact\tUnknown\nTRUE\tTRUE\tNULL\n"
    );
    for sql in ["SELECT 1 IN (1,1/0)", "SELECT 'secret' IN ('secret')"] {
        let result = run(&["query", "eval", sql]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("secret"));
    }
}

#[test]
fn builtins_share_scalar_and_retrieval_results_without_partial_error_output() {
    let sql = "SELECT IsNull(Null) AS MissingValue, IsNull('') AS EmptyText, IIf(Null,1,2) AS Chosen, IIf(9223372036854775807,'yes','no') AS Exact";
    for command in ["eval", "run"] {
        let result = run(&["query", command, sql]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"MissingValue\tEmptyText\tChosen\tExact\nTRUE\tFALSE\t2\t\"\"\"yes\"\"\"\n"
        );
        for sql in [
            "SELECT IIf(True,1,1/0)",
            "SELECT IIf(True,1)",
            "SELECT VBA.IIf(True,1,0)",
        ] {
            let failure = run(&["query", command, sql]);
            assert_eq!(failure.status.code(), Some(2));
            assert!(failure.stdout.is_empty());
        }
    }
}

#[test]
fn nz_query_context_is_the_same_in_eval_and_run() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT IsNull(Nz(Null)) AS EmptyDefault, Nz(Null,Null) AS ExplicitNull, Nz(Null,42) AS Fallback",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"EmptyDefault\tExplicitNull\tFallback\nFALSE\tNULL\t42\n"
        );
        let result = run(&["query", command, "SELECT Nz(1,1/0)"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn common_text_functions_use_shared_unicode_rules_without_partial_output() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT Len('é🦀') AS Units, Left('é🦀',1) AS Prefix, IsNull(Trim(Null)) AS NullResult, Len(Trim('  x  ')) AS Trimmed",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            "Units\tPrefix\tNullResult\tTrimmed\n3\t\"\"\"é\"\"\"\tTRUE\t1\n".as_bytes()
        );
        for sql in [
            "SELECT 1,Left('🦀',1)",
            "SELECT Mid('abc',0)",
            "SELECT Len(*)",
        ] {
            let result = run(&["query", command, sql]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}
