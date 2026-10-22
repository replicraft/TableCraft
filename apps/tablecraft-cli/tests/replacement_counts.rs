mod support;
use support::run;

#[test]
fn cli_replacement_counts_preserve_suffixes_and_buffer_conversion_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Replace('banana','an','X','2,5','2,5'),Replace('banana','an','X',CDate(2.5),CDate(-0.5))",
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
        assert_eq!(row.iter().collect::<Vec<_>>(), ["\"XXa\"", "\"anana\""]);
        for sql in [
            "SELECT 99,Replace('abc','a','b','1')",
            "SELECT 99,Replace(NULL,'a','b','bad',1/0)",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}
