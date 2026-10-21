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
fn system_calendar_options_keep_omitted_defaults_and_sql_overrides() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT DatePart('w',#2026-10-20#,0) AS W,DatePart('w',#2026-10-20#) AS D,DatePart('ww',#2021-01-01#,0,0) AS N,DatePart('ww',#2021-01-01#,,) AS O,DateDiff('ww',#2026-10-18#,#2026-10-19#,0,0) AS X,DateDiff('ww',#2026-10-18#,#2026-10-19#) AS Y,DateDiff('ww',#2026-10-18#,#2026-10-19#,1,1) AS Z",
            "--first-day-of-week",
            "2",
            "--first-week-of-year",
            "2",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            rows(&result.stdout),
            [
                vec!["W", "D", "N", "O", "X", "Y", "Z"],
                vec!["2", "3", "53", "1", "1", "0", "0"]
            ]
        );
    }
}
#[test]
fn missing_host_policy_null_options_and_invalid_flags_publish_nothing() {
    for mode in ["eval", "run"] {
        for expression in [
            "DatePart('ww',#2021-01-01#,0,0)",
            "DatePart('ww',NULL,0,0)",
            "DatePart('w',#2026-10-20#,NULL)",
            "DateDiff('ww',#2026-10-18#,#2026-10-19#,0,0)",
            "DatePart('ww',#2021-01-01#,0,1/0)",
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--first-day-of-week",
                "2",
            ]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
        for rule in ["0", "4", "four-day"] {
            let result = run(&["query", mode, "SELECT 1", "--first-week-of-year", rule]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
    let result = run(&["query", "parse", "SELECT 1", "--first-week-of-year", "2"]);
    assert!(!result.status.success());
}
