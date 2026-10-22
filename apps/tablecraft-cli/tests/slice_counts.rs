mod support;
use support::run;

#[test]
fn cli_slice_counts_share_numeric_text_policies_and_buffer_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Left('abcd',1.5),Right('abcd',CDec(2.5)),Mid('abcd','1,5','2,5')",
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
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            ["\"ab\"", "\"cd\"", "\"bc\""]
        );
        for sql in [
            "SELECT 99,Left('abc','1.5')",
            "SELECT 99,Left('é🦀中',2.5)",
            "SELECT Left(NULL,1/0)",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}
