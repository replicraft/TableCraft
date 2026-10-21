mod support;
use support::run;
fn table(bytes: &[u8]) -> Vec<Vec<String>> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(bytes)
        .records()
        .map(|row| row.unwrap().iter().map(str::to_owned).collect())
        .collect()
}
#[test]
fn date_conversion_runs_with_separate_year_window_and_reference_year_controls() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Year(DateValue('Feb 29')) AS Y,Hour(TimeValue('2:24PM')) AS H,Year(CDate('1/2/49')) AS W,DateValue(NULL) AS N",
            "--date-format",
            "en-us",
            "--reference-year",
            "2024",
            "--two-digit-year-max",
            "2049",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            table(&result.stdout),
            [vec!["Y", "H", "W", "N"], vec!["2024", "14", "2049", "NULL"]]
        );
        let result = run(&[
            "query",
            mode,
            "SELECT Hour(CDate('-1.25')) AS H",
            "--date-format",
            "iso",
            "--number-format",
            "invariant",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(table(&result.stdout), [vec!["H"], vec!["6"]]);
        for expression in [
            "CDate(NULL)",
            "DateValue('2026-01-01')",
            "DateValue('Feb 29')",
            "TimeValue('2026-02-30 12:00')",
            "CDate(1,1/0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
#[test]
fn reference_year_is_validated_and_not_available_during_inert_parse() {
    for args in [
        vec!["query", "eval", "SELECT 1", "--reference-year", "99"],
        vec!["query", "parse", "SELECT 1", "--reference-year", "2024"],
    ] {
        let result = run(&args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}
