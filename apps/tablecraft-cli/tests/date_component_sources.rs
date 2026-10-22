mod support;
use support::run;

#[test]
fn cli_component_sources_use_explicit_date_number_policies_and_buffer_failures() {
    for mode in ["eval", "run"] {
        let sql = "SELECT Year('2024-02-29'),Month('2024-02-29'),Hour('45658,5'),Hour(-1.25)";
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
        assert_eq!(row.iter().collect::<Vec<_>>(), ["2024", "2", "12", "6"]);
        for sql in [
            "SELECT 99,Year('2024-02-29')",
            "SELECT 99,Year(2958466)",
            "SELECT 99,Year(NULL),1/0",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}
