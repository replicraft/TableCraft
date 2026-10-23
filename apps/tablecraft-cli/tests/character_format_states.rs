mod support;
use support::run;

#[test]
fn character_and_format_error_states_keep_explicit_conversion_and_buffered_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Chr(CLng(CVErr(65))) AS C,FormatNumber(CLng(CVErr(11)),0) AS N",
            "--number-format",
            "en-US",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let records: Vec<Vec<String>> = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(result.stdout.as_slice())
            .records()
            .map(|record| record.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(records, [vec!["C", "N"], vec!["\"A\"", "\"11\""]]);

        for (expression, diagnostic) in [
            ("Chr(CVErr(65))", "type mismatch"),
            ("Chr$(CVErr(65))", "type mismatch"),
            ("ChrW(CVErr(65))", "type mismatch"),
            ("FormatNumber(1,256,CVErr(11))", "domain"),
            ("FormatNumber(NULL,CVErr(11))", "invalid use of NULL"),
            ("FormatNumber(CVErr(11),1/0)", "division by zero"),
            ("FormatPercent(1,CVErr(11))", "type mismatch"),
            ("FormatCurrency(CVErr(11))", "type mismatch"),
        ] {
            let sql = format!("SELECT 1,{expression}");
            let result = run(&["query", mode, &sql, "--number-format", "en-US"]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty(), "{sql}");
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(diagnostic),
                "{sql}"
            );
        }
    }
}
