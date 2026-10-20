mod support;
use support::run;
#[test]
fn numeric_text_profiles_work_in_both_paths_and_do_not_change_val() {
    for mode in ["eval", "run"] {
        for (format, text) in [("en-us", "$1,234.50"), ("de-de", "1.234,50 €")] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT CDbl('{text}') AS N,CLng('{text}') AS I,Val('1,234') AS V"),
                "--number-format",
                format,
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let rows: Vec<Vec<String>> = csv::ReaderBuilder::new()
                .delimiter(b'\t')
                .has_headers(false)
                .from_reader(result.stdout.as_slice())
                .records()
                .map(|row| row.unwrap().iter().map(str::to_owned).collect())
                .collect();
            assert_eq!(rows[0], ["N", "I", "V"]);
            assert_eq!(rows[1][0].parse::<f64>().unwrap(), 1234.5);
            assert_eq!(&rows[1][1..], ["1234", "1"]);
        }
        let result = run(&[
            "query",
            mode,
            "SELECT CLngLng('9223372036854775807'),CBool('TrUe'),CByte('255')",
            "--number-format",
            "invariant",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        for expression in [
            "CLng('12tail')",
            "CInt('1,23.0')",
            "CCur('922337203685477.5808')",
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--number-format",
                "en-us",
            ]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
        let result = run(&["query", mode, "SELECT CLng('1')"]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("number-format policy"));
    }
}
#[test]
fn numeric_format_is_only_an_execution_option() {
    for args in [
        vec!["query", "eval", "SELECT 1", "--number-format", "unknown"],
        vec!["query", "parse", "SELECT 1", "--number-format", "en-us"],
    ] {
        let result = run(&args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}
