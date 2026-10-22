mod support;
use support::run;

#[test]
fn cli_numeric_date_sources_use_serials_and_buffer_null_or_eager_failures() {
    for mode in ["eval", "run"] {
        let sql = concat!(
            "SELECT FormatNumber(#1899-12-29 06:00:00#),",
            "FormatPercent(CDate(-0.25)),FormatCurrency(#2025-01-01 12:00:00#)"
        );
        let result = run(&["query", mode, sql, "--number-format", "en-US"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            ["\"-1.25\"", "\"-25.00%\"", "\"$45,658.50\""]
        );
        for sql in [
            "SELECT 99,FormatNumber(NULL)",
            "SELECT 99,FormatNumber(#2025-01-01#),1/0",
        ] {
            let result = run(&["query", mode, sql, "--number-format", "en-US"]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}
