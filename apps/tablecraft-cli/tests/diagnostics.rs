mod support;
use support::run;

#[test]
fn verbose_diagnostics_keep_stdout_and_omit_query_payloads() {
    let sql = "SELECT 'private-value-91' AS [private-column-37]";
    let quiet = run(&["query", "run", sql]);
    let verbose = run(&["-v", "query", "run", sql]);
    assert!(quiet.status.success() && verbose.status.success());
    assert!(quiet.stderr.is_empty());
    assert_eq!(quiet.stdout, verbose.stdout);
    let logs = String::from_utf8(verbose.stderr).unwrap();
    for span in [
        "sql.parse",
        "query.bind",
        "query.execute",
        "query completed",
    ] {
        assert!(logs.contains(span), "{logs}");
    }
    for payload in [sql, "private-value-91", "private-column-37"] {
        assert!(!logs.contains(payload));
    }
    let failure = run(&["-v", "query", "run", "SELECT 1 / 0 AS [private-error-12]"]);
    assert_eq!(failure.status.code(), Some(2));
    assert!(failure.stdout.is_empty());
    let logs = String::from_utf8(failure.stderr).unwrap();
    assert!(logs.contains("success=false"));
    assert!(!logs.contains("private-error-12"));
}
#[test]
fn trace_detail_omits_native_paths_and_names() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let path = path.to_str().unwrap();
    let sql = "SELECT TOP 1 ORDR_NBR FROM ROULETTE_TB ORDER BY ORDR_NBR";
    let debug = run(&["-v", "query", "run", "--database", path, sql]);
    let trace = run(&["-vv", "query", "run", "--database", path, sql]);
    assert!(debug.status.success() && trace.status.success());
    assert_eq!(debug.stdout, trace.stdout);
    let debug = String::from_utf8(debug.stderr).unwrap();
    let trace = String::from_utf8(trace.stderr).unwrap();
    assert!(!debug.contains("io.read_at"));
    assert!(trace.contains("io.read_at"));
    for payload in [path, "Access_Example_VBA", "ROULETTE_TB", "ORDR_NBR", sql] {
        assert!(!trace.contains(payload));
    }
}
