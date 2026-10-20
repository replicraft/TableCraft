mod support;
use support::run;

#[test]
fn numeric_prefixes_execute_in_both_paths_and_errors_do_not_leak_output() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Val(' 2 45 7') AS A,Val('&HFFFF') AS B,Val('24 and 57') AS C,Val('1.23455@') AS D",
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
        assert_eq!(rows[0], ["A", "B", "C", "D"]);
        let values: Vec<f64> = rows[1].iter().map(|cell| cell.parse().unwrap()).collect();
        assert_eq!(values, [2457.0, -1.0, 24.0, 1.2346]);
        for expr in [
            "Val(NULL)",
            "Val(12)",
            "Val('50.5%')",
            "Val('1e400')",
            "Val('1',1/0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expr}")]);
            assert!(!result.status.success(), "{expr}");
            assert!(result.stdout.is_empty(), "{expr}");
        }
    }
}
