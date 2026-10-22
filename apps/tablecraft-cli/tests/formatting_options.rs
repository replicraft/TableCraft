mod support;
use support::run;

#[test]
fn cli_format_options_share_explicit_number_and_date_profiles() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT FormatNumber(1,'2,5'),FormatDateTime(#2025-01-02 03:04:05#,'2,5')",
            "--number-format",
            "de-DE",
            "--date-format",
            "iso",
        ]);
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
            ["\"1,00\"", "\"2025-01-02\""]
        );
        for sql in [
            "SELECT 99,FormatDateTime(#2025-01-02#,'2')",
            "SELECT FormatNumber(NULL,'bad',1/0)",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}
