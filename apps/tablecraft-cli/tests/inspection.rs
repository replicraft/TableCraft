mod support;
use support::run;
fn table(bytes: &[u8]) -> Vec<Vec<String>> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(bytes)
        .records()
        .map(|record| record.unwrap().iter().map(str::to_owned).collect())
        .collect()
}
#[test]
fn numeric_inspection_works_in_both_paths_and_failures_publish_nothing() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT IsNumeric(0) AS N,IsNumeric(False) AS B,IsNumeric(NULL) AS Z,IsNumeric(#2026-10-20#) AS D",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            table(&result.stdout),
            [
                vec!["N", "B", "Z", "D"],
                vec!["TRUE", "TRUE", "FALSE", "FALSE"]
            ]
        );
        let result = run(&[
            "query",
            mode,
            "SELECT IsNumeric('53') AS A,IsNumeric('45 Help') AS B,IsNumeric('$1,234.50') AS C,IsNumeric('1e999') AS D",
            "--number-format",
            "en-us",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            table(&result.stdout),
            [
                vec!["A", "B", "C", "D"],
                vec!["TRUE", "FALSE", "TRUE", "FALSE"]
            ]
        );
        for expression in [
            "IsNumeric('53')",
            "IsNumeric(1/0)",
            "IsNumeric('1',1/0)",
            "VBA.IsNumeric(1)",
            "IIf(False,IsNumeric('1'),0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn date_inspection_uses_shared_flags_and_buffers_eager_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT IsDate('04/28/2014') AS D,IsDate('13/32/2014') AS X,IsDate(NULL) AS N,IsDate('Feb 29') AS L,IsDate(#2026-10-20#) AS T",
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
        assert_eq!(
            table(&result.stdout),
            [
                vec!["D", "X", "N", "L", "T"],
                vec!["TRUE", "FALSE", "FALSE", "TRUE", "TRUE"]
            ]
        );
        for expression in [
            "IsDate('2026-10-20')",
            "IsDate(1/0)",
            "IsDate()",
            "IsDate(*)",
            "IsDate(1,1/0)",
            "VBA.IsDate(1)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
        let result = run(&[
            "query",
            mode,
            "SELECT 1,IsDate('Feb 29')",
            "--date-format",
            "en-us",
        ]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}
