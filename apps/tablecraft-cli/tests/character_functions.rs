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
fn character_functions_work_in_both_query_paths_with_exact_carriers_and_buffered_errors() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Chr(65) AS A,Asc('Apple') AS B,ChrW(8364) AS C,AscW('😀') AS D",
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
                vec!["\"A\"", "65", "\"€\"", "-10179"]
            ]
        );
        let result = run(&[
            "query",
            mode,
            "SELECT Chr(128) AS A,Asc('€') AS B",
            "--character-code-page",
            "1252",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            table(&result.stdout),
            [vec!["A", "B"], vec!["\"€\"", "128"]]
        );
        for expression in [
            "Chr(128)",
            "Asc('é')",
            "Asc('')",
            "Chr(NULL)",
            "AscW(NULL)",
            "ChrW(55296)",
            "Chr(256)",
            "Chr(65,1/0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
        let result = run(&[
            "query",
            mode,
            "SELECT 1,Asc('中')",
            "--character-code-page",
            "1252",
        ]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}
