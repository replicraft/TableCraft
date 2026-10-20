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
fn cstr_uses_separate_numeric_and_date_policies_in_both_query_paths() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT CStr(True) AS B,CStr('1.23') AS T,CStr(1.25) AS N,Str(1.25) AS S,CStr(#2026-10-20 13:04:05#) AS D",
            "--number-format",
            "de-de",
            "--date-format",
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
                vec!["B", "T", "N", "S", "D"],
                vec![
                    "\"True\"",
                    "\"1.23\"",
                    "\"1,25\"",
                    "\" 1.25\"",
                    "\"10/20/2026 1:04:05 PM\""
                ]
            ]
        );
        for expression in [
            "CStr(NULL)",
            "CStr(1)",
            "CStr(#2026-10-20#)",
            "CStr('x',1/0)",
            "IIf(False,CStr(NULL),'x')",
            "VBA.CStr(1)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
#[test]
fn date_format_is_only_an_execution_option_and_is_validated() {
    for args in [
        vec!["query", "eval", "SELECT 1", "--date-format", "unknown"],
        vec!["query", "parse", "SELECT 1", "--date-format", "iso"],
    ] {
        let result = run(&args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}
