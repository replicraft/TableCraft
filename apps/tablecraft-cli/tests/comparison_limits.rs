mod support;
use support::run;

#[test]
fn comparison_allowances_have_explicit_ranges_and_require_a_policy() {
    for (flag, maximum) in [
        ("--comparison-bytes", "268435456"),
        ("--comparison-steps", "100000000"),
    ] {
        for value in ["0", "-1", "many", "18446744073709551616"] {
            let result = run(&[
                "query",
                "run",
                "SELECT 1",
                "--collation",
                "general-2010",
                flag,
                value,
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
        let result = run(&["query", "run", "SELECT 1", flag, maximum]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("--collation"));
        let result = run(&[
            "query",
            "run",
            "SELECT 'a'='A' AS E",
            "--collation",
            "general-2010",
            flag,
            maximum,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"E\nTRUE\n");
    }
    for (flag, value) in [
        ("--comparison-bytes", "268435457"),
        ("--comparison-steps", "100000001"),
    ] {
        let result = run(&[
            "query",
            "run",
            "SELECT 1",
            "--collation",
            "general-2010",
            flag,
            value,
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn low_comparison_pools_fail_buffered_without_borrowing_execution_allowances() {
    for flag in ["--comparison-bytes", "--comparison-steps"] {
        let result = run(&[
            "query",
            "run",
            "SELECT 'a'='A'",
            "--collation",
            "general-2010",
            flag,
            "1",
            "--execution-bytes",
            "268435456",
            "--execution-steps",
            "100000000",
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("resource limit"));
    }
}
