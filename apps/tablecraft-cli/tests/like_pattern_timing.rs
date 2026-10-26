mod support;
use support::run;

#[test]
fn cli_like_timing_returns_early_results_instead_of_unreached_pattern_errors() {
    for (mode, star) in [("ansi89", "*"), ("ansi92", "%")] {
        for declared in [false, true] {
            let sql = format!(
                "SELECT 'x' LIKE 'a[' AS A,'x' NOT LIKE 'a[z-a]' AS B,'' LIKE '[' AS C,'' LIKE '[]{star}' AS D,NULL LIKE '{star}[' AS E"
            );
            let mut args = vec![
                "query",
                "run",
                &sql,
                "--like-mode",
                mode,
                "--collation",
                "ordinal",
            ];
            if declared {
                args.push("--declared-arithmetic");
            }
            let output = run(&args);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                output.stdout,
                b"A\tB\tC\tD\tE\nFALSE\tTRUE\tFALSE\tTRUE\tNULL\n"
            );
        }
    }
}

#[test]
fn cli_reached_pattern_and_eager_operand_errors_still_buffer_stdout() {
    for (mode, star) in [("ansi89", "*"), ("ansi92", "%")] {
        for (sql, diagnostic) in [
            (
                "SELECT 1 AS Prefix,'ab' LIKE 'a[' AS M".to_owned(),
                "invalid LIKE pattern",
            ),
            (
                format!("SELECT 1 AS Prefix,'x' NOT LIKE '{star}a[' AS M"),
                "invalid LIKE pattern",
            ),
            (
                "SELECT 1 AS Prefix,NULL LIKE CStr(1/0) AS M".to_owned(),
                "division by zero",
            ),
        ] {
            let output = run(&[
                "query",
                "run",
                &sql,
                "--like-mode",
                mode,
                "--collation",
                "ordinal",
            ]);
            assert!(!output.status.success());
            assert!(output.stdout.is_empty(), "{sql}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(diagnostic),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
