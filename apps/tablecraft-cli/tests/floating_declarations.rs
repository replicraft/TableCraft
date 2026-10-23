#[path = "support/mod.rs"]
mod cli_support;
use cli_support::run;

#[test]
fn declared_arithmetic_and_legacy_alias_keep_single_failures_buffered() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let result = run(&[
            "query",
            "run",
            "SELECT 1,CSng(3.4028234663852886e38)*CInt(2)",
            flag,
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("overflow"));
        let result = run(&[
            "query",
            "run",
            "SELECT VarType(CVar(CSng(3.4028234663852886e38))*CInt(2)) AS T",
            flag,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"T\n5\n");
    }
    let result = run(&[
        "query",
        "run",
        "SELECT VarType(CSng(3.4028234663852886e38)*CInt(2)) AS T",
    ]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"T\n5\n");
}
