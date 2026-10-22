mod support;
use support::run;

#[test]
fn cli_week_options_use_explicit_number_syntax_and_buffer_failures() {
    let sql = concat!(
        "SELECT DatePart('ww',#2021-01-01#,'2,5','1,5'),",
        "DateDiff('ww',#2026-10-18#,#2026-10-19#,'1,5'),",
        "Weekday(#2025-01-01#,'2,5'),",
        "Format(#2021-01-01#,'w ww','2,5','1,5')"
    );
    for mode in ["eval", "run"] {
        let result = run(&["query", mode, sql, "--number-format", "de-DE"]);
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
        assert_eq!(row.iter().collect::<Vec<_>>(), ["53", "1", "3", "\"5 53\""]);
        for query in [
            "SELECT 99,Weekday(#2025-01-01#,'2.5')",
            "SELECT 99,DatePart('ww',NULL,'8',1/0)",
            "SELECT 99,Format(NULL,'0','8',1/0)",
        ] {
            let result = run(&["query", mode, query]);
            assert!(!result.status.success(), "{query}");
            assert!(result.stdout.is_empty());
        }
    }
}
