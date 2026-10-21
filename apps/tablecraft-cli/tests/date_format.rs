mod support;
use support::run;
fn rows(bytes: &[u8]) -> Vec<Vec<String>> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(bytes)
        .records()
        .map(|record| record.unwrap().iter().map(str::to_owned).collect())
        .collect()
}
#[test]
fn all_named_formats_and_optional_slot_work_in_both_cli_paths() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT FormatDateTime(#2026-10-20 16:28:14#) AS G,FormatDateTime(#2026-10-20#,1) AS L,FormatDateTime(0,2) AS D,FormatDateTime('04.28.14',3) AS T,FormatDateTime(#2026-10-20 16:28:14#,4) AS S,FormatDateTime(#2026-10-20#,) AS M",
            "--date-format",
            "en-us",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            rows(&result.stdout),
            [
                vec!["G", "L", "D", "T", "S", "M"],
                vec![
                    "\"10/20/2026 4:28:14 PM\"",
                    "\"Tuesday, October 20, 2026\"",
                    "\"12/30/1899\"",
                    "\"4:28:14 AM\"",
                    "\"16:28\"",
                    "\"10/20/2026\""
                ]
            ]
        );
        let result = run(&[
            "query",
            mode,
            "SELECT FormatDateTime('Feb 29',2)",
            "--date-format",
            "en-us",
            "--reference-year",
            "2024",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(rows(&result.stdout)[1], ["\"2/29/2024\""]);
    }
}
#[test]
fn errors_and_invalid_call_shapes_publish_nothing() {
    for mode in ["eval", "run"] {
        for expression in [
            "FormatDateTime(NULL)",
            "FormatDateTime(1,5)",
            "FormatDateTime(1,NULL)",
            "FormatDateTime()",
            "FormatDateTime(*)",
            "FormatDateTime(,2)",
            "FormatDateTime(1,1,1/0)",
            "VBA.FormatDateTime(1)",
            "FormatDateTime(1/0)",
            "IIf(False,FormatDateTime(1/0),0)",
            "FormatDateTime('Feb 29')",
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--date-format",
                "en-us",
            ]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
        let result = run(&["query", mode, "SELECT 1,FormatDateTime(#2026-10-20#)"]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}
