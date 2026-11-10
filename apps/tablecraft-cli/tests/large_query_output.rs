use tablecraft_testkit::support::cli::Runner;

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
fn omitted_text_quota_accepts_long_function_results_and_explicit_lower_quota_refuses() {
    let sql = "SELECT Len(String(70000,'a')) AS N";
    let default = CLI.run(&["query", "run", sql]);
    assert!(
        default.status.success(),
        "{}",
        String::from_utf8_lossy(&default.stderr)
    );
    assert_eq!(default.stdout, b"N\n70000\n");
    let eval = CLI.run(&["query", "eval", sql]);
    assert!(
        eval.status.success(),
        "{}",
        String::from_utf8_lossy(&eval.stderr)
    );
    assert_eq!(eval.stdout, b"N\n70000\n");
    let limited = CLI.run(&["query", "run", sql, "--text-bytes", "69999"]);
    assert_eq!(limited.status.code(), Some(2));
    assert!(limited.stdout.is_empty());
    let allowed = CLI.run(&["query", "run", sql, "--text-bytes", "70000"]);
    assert!(
        allowed.status.success(),
        "{}",
        String::from_utf8_lossy(&allowed.stderr)
    );
    assert_eq!(allowed.stdout, b"N\n70000\n");
    for command in ["eval", "parse", "saved", "update"] {
        let rejected = CLI.run(&["query", command, sql, "--text-bytes", "70000"]);
        assert_eq!(rejected.status.code(), Some(2));
        assert!(rejected.stdout.is_empty());
    }
}

#[test]
fn output_limit_applies_to_intermediate_display_and_final_csv_expansion() {
    // Each displayed quote is escaped; CSV adds another quote per original quote.
    let sql = "SELECT String(400000,34) AS S";
    let options = [
        "query",
        "run",
        sql,
        "--text-bytes",
        "400000",
        "--execution-steps",
        "10000000",
        "--format",
        "tsv",
    ];
    let default = CLI.run(&options);
    assert!(
        default.status.success(),
        "{}",
        String::from_utf8_lossy(&default.stderr)
    );
    assert!(default.stdout.len() > 1_048_576);
    let mut args = options.to_vec();
    args.extend(["--output-bytes", "1300000"]);
    let allowed = CLI.run(&args);
    assert!(
        allowed.status.success(),
        "{}",
        String::from_utf8_lossy(&allowed.stderr)
    );
    assert_eq!(allowed.stdout, default.stdout);
    assert!(allowed.stdout.len() > 1_048_576 && allowed.stdout.len() < 1_300_000);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(allowed.stdout.as_slice());
    let rows = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get(0),
        Some(format!("\"{}\"", "\\\"".repeat(400_000)).as_str())
    );
    let mut args = options.to_vec();
    args.extend(["--output-bytes", "100"]);
    let too_small = CLI.run(&args);
    assert_eq!(too_small.status.code(), Some(2));
    assert!(too_small.stdout.is_empty());
}

#[test]
fn large_text_and_output_ranges_are_checked_before_execution() {
    for (flag, value) in [
        ("--text-bytes", "0"),
        ("--text-bytes", "18446744073709551615"),
        ("--output-bytes", "0"),
        ("--output-bytes", "18446744073709551615"),
    ] {
        let rejected = CLI.run(&["query", "run", "SELECT 1", flag, value]);
        assert_eq!(rejected.status.code(), Some(2));
        assert!(rejected.stdout.is_empty());
    }
    for (flag, value) in [("--text-bytes", "16777217"), ("--output-bytes", "16777217")] {
        let allowed = CLI.run(&["query", "run", "SELECT 1 AS N", flag, value]);
        assert!(allowed.status.success());
        assert_eq!(allowed.stdout, b"N\n1\n");
    }
}
