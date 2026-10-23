#[path = "support/mod.rs"]
mod cli_support;
use cli_support::run;

#[test]
fn declared_integral_flag_preserves_defaults_and_keeps_failures_buffered() {
    let sql = "SELECT 1,CInt(32767)+CInt(1)";
    let default = run(&["query", "run", sql]);
    assert!(default.status.success());
    let strict = run(&["query", "run", sql, "--declared-integral"]);
    assert_eq!(strict.status.code(), Some(2));
    assert!(strict.stdout.is_empty());
    assert!(String::from_utf8_lossy(&strict.stderr).contains("overflow"));
    let variant = run(&[
        "query",
        "run",
        "SELECT Q.N+CInt(1) AS N FROM (SELECT CVar(CInt(32767)) AS N) AS Q",
        "--declared-integral",
    ]);
    assert!(
        variant.status.success(),
        "{}",
        String::from_utf8_lossy(&variant.stderr)
    );
    assert_eq!(variant.stdout, b"N\n32768\n");
    let unknown = run(&[
        "query",
        "run",
        "SELECT Abs(1)+CInt(1)",
        "--declared-integral",
    ]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("declaration is unknown"));
}

#[test]
fn declared_integral_flag_is_scoped_to_query_execution() {
    for mode in ["eval", "parse", "saved", "update"] {
        let result = run(&["query", mode, "SELECT 1", "--declared-integral"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    let help = run(&["query", "run", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--declared-arithmetic"));
    assert!(String::from_utf8_lossy(&help.stdout).contains("declared-integral"));
}

#[test]
fn rounded_operand_declarations_keep_scalar_overflow_and_variant_widening() {
    for (expression, succeeds) in [
        ("CInt(-32768)\\CDbl(-1)", false),
        ("CVar(CInt(-32768))\\CDbl(-1)", true),
        ("CInt(-32768)\\CVar(CDbl(-1))", true),
    ] {
        let sql = format!("SELECT {expression} AS N");
        let result = run(&["query", "run", &sql, "--declared-integral"]);
        assert_eq!(
            result.status.success(),
            succeeds,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        if succeeds {
            assert_eq!(result.stdout, b"N\n32768\n");
        } else {
            assert!(result.stdout.is_empty());
        }
    }
}
