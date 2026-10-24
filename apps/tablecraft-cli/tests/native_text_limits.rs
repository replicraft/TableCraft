#[path = "support/mod.rs"]
mod cli_support;

use cli_support::run;

#[test]
fn native_text_limits_are_bounded_and_database_scoped_before_io() {
    for value in ["0", "16777217", "4294967295"] {
        let output = run(&[
            "query",
            "run",
            "SELECT 1",
            "--database",
            "missing.accdb",
            "--native-text-bytes",
            value,
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("invalid value"));
    }
    let output = run(&["query", "run", "SELECT 1", "--native-text-bytes", "1048577"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--database"));

    let output = run(&[
        "query",
        "run",
        "SELECT 1",
        "--sqlite",
        "missing.sqlite",
        "--native-text-bytes",
        "1048577",
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn larger_generated_query_text_keeps_query_and_output_allowances_separate() {
    let sql = "SELECT Len(String(1048577,'a')) AS N";
    let options = [
        "query",
        "run",
        sql,
        "--text-bytes",
        "1048577",
        "--execution-steps",
        "10000000",
    ];
    let output = run(&options);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"N\n1048577\n");
    let mut low = options.to_vec();
    low.extend(["--execution-bytes", "1000"]);
    let output = run(&low);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
