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
fn names_optional_slots_and_first_day_override_work_in_both_paths() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT MonthName(3) AS M,MonthName(3,True) AS A,MonthName(1,) AS O,WeekdayName(1) AS D,WeekdayName(1,,1) AS S,WeekdayName(1,True,0) AS B,WeekdayName(1,,) AS N",
            "--date-format",
            "de-de",
            "--first-day-of-week",
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
                vec!["M", "A", "O", "D", "S", "B", "N"],
                vec![
                    "\"März\"",
                    "\"Mär\"",
                    "\"Januar\"",
                    "\"Montag\"",
                    "\"Sonntag\"",
                    "\"Mo\"",
                    "\"Montag\""
                ]
            ]
        );
        let result = run(&[
            "query",
            mode,
            "SELECT MonthName(1.5,True),WeekdayName(7,True,7)",
            "--date-format",
            "en-us",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(rows(&result.stdout)[1], ["\"Feb\"", "\"Fri\""]);
    }
}
#[test]
fn missing_policy_invalid_arguments_and_flag_values_publish_nothing() {
    for mode in ["eval", "run"] {
        for expression in [
            "MonthName(NULL)",
            "MonthName(0)",
            "MonthName(13)",
            "MonthName(1,NULL)",
            "MonthName(*)",
            "MonthName(,True)",
            "MonthName(1,False,1/0)",
            "WeekdayName(1)",
            "WeekdayName(8,False,1)",
            "WeekdayName(1,False,8)",
            "WeekdayName(1,False,NULL)",
            "VBA.MonthName(1)",
            "MonthName(1/0)",
            "IIf(False,MonthName(1/0),'x')",
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
        let result = run(&["query", mode, "SELECT 1,MonthName(1)"]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        for day in ["0", "8", "Monday"] {
            let result = run(&["query", mode, "SELECT 1", "--first-day-of-week", day]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
    let result = run(&["query", "parse", "SELECT 1", "--first-day-of-week", "2"]);
    assert!(!result.status.success());
}
