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

#[test]
fn cli_slice_states_preserve_eager_failures_null_sources_and_explicit_cstr() {
    for mode in ["eval", "run"] {
        for (expression, error) in [
            ("Left(CVErr(1),1/0)", "division by zero"),
            ("Right(CVErr(1),1/0)", "division by zero"),
            ("Mid('abc',CVErr(1),1/0)", "division by zero"),
            ("Mid('abc',0,CVErr(1))", "domain"),
            ("Mid('abc',1,CVErr(1))", "type mismatch"),
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2), "{expression}");
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(error));
        }
        let result = run(&[
            "query",
            mode,
            "SELECT Left(NULL,CVErr(1)),Right(NULL,CVErr(1)),Mid(NULL,0,CVErr(1)),Right(CStr(CVErr(11)),2)",
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
            ["NULL", "NULL", "NULL", "\"11\""]
        );
    }
}
