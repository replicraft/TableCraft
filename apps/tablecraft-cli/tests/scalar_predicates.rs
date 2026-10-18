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
