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
fn weekday_supports_defaults_optional_slots_conversion_and_host_context() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Weekday(#1969-02-12#) AS A,Weekday(#1969-02-12#,2) AS B,Weekday(#1969-02-12#,0) AS C,Weekday(#1969-02-12#,) AS D,Weekday(NULL) AS N,Weekday(0) AS E,Weekday('Feb 29',2) AS T,Weekday('-1.25') AS F",
            "--first-day-of-week",
            "2",
            "--date-format",
            "en-us",
            "--reference-year",
            "2024",
            "--number-format",
            "invariant",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            rows(&result.stdout),
            [
                vec!["A", "B", "C", "D", "N", "E", "T", "F"],
                vec!["4", "3", "3", "4", "NULL", "7", "4", "6"]
            ]
        );
    }
}
#[test]
fn invalid_shapes_missing_policies_and_eager_failures_publish_nothing() {
    for mode in ["eval", "run"] {
        for expression in [
            "Weekday()",
            "Weekday(*)",
            "Weekday(,2)",
            "Weekday(1,1,1/0)",
            "VBA.Weekday(1)",
            "Weekday('2024-02-29')",
            "Weekday(1,0)",
            "Weekday(1,8)",
            "Weekday(1,NULL)",
            "Weekday(NULL,1/0)",
            "IIf(False,Weekday(1/0),0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
