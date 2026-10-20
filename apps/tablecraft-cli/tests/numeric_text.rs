mod support;
use support::run;
#[test]
fn numeric_text_calls_work_in_both_paths_with_buffered_errors() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Hex(459) AS H,Oct(459) AS O,Str(-459.65) AS S,Str(459.001) AS P,Hex(NULL) AS N",
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
        assert_eq!(
            rows,
            [
                vec!["H", "O", "S", "P", "N"],
                vec!["\"1CB\"", "\"713\"", "\"-459.65\"", "\" 459.001\"", "NULL"]
            ]
        );
        if mode == "eval" {
            let oversized = format!("SELECT {}", vec!["Str(CDbl(1e-299))"; 16].join(","));
            let result = run(&["query", mode, &oversized]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
        for expression in ["Hex('x')", "Oct(1,1/0)", "Str('1')", "Hex(2147483648.0)"] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
