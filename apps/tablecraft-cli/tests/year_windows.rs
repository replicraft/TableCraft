mod support;
use support::run;

#[test]
fn both_cli_paths_use_explicit_windows_and_keep_the_documented_default() {
    for mode in ["eval", "run"] {
        let sql = "SELECT Year(DateSerial(49,1,1)) AS A,Year(DateSerial(50,1,1)) AS B,Year(DateSerial(1949,1,1)) AS C";
        for (extra, expected) in [
            (vec![], &b"A\tB\tC\n1949\t1950\t1949\n"[..]),
            (
                vec!["--two-digit-year-max", "2049"],
                &b"A\tB\tC\n2049\t1950\t1949\n"[..],
            ),
        ] {
            let mut args = vec!["query", mode, sql];
            args.extend(extra);
            let result = run(&args);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, expected);
            assert!(result.stderr.is_empty());
        }
    }
}
#[test]
fn invalid_window_flags_fail_before_native_io_and_errors_remain_buffered() {
    for value in ["198", "10000", "65536", "wrong"] {
        let result = run(&[
            "query",
            "run",
            "SELECT DateSerial(49,1,1)",
            "--database",
            "does-not-exist.accdb",
            "--two-digit-year-max",
            value,
        ]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("100-year window"));
    }
    let result = run(&[
        "query",
        "eval",
        "SELECT 1 AS Before,DateSerial(NULL,1,1)",
        "--two-digit-year-max",
        "2049",
    ]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    let result = run(&[
        "query",
        "run",
        "SELECT Year(DateSerial(99,1,1)) AS N",
        "--two-digit-year-max",
        "199",
    ]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"N\n199\n");
}
