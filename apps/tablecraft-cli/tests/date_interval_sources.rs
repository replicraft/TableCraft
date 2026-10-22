mod support;
use support::run;

#[test]
fn cli_date_interval_sources_share_explicit_policies_and_buffer_failures() {
    let sql = concat!(
        "SELECT Day(DateAdd('m','1','2024-01-31')),",
        "DatePart('h','-1,25'),DateDiff('h','-1,25',0)"
    );
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            sql,
            "--date-format",
            "iso",
            "--number-format",
            "de-DE",
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
        assert_eq!(row.iter().collect::<Vec<_>>(), ["29", "6", "18"]);
        for sql in [
            "SELECT 99,DateAdd('d',1,'2025-01-01')",
            "SELECT 99,DatePart('d','2025-01-01')",
            "SELECT 99,DateDiff('d',NULL,'2025-01-01')",
            "SELECT 99,DateAdd('bad',NULL,1/0)",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}
