mod support;

use support::run;

#[test]
fn packed_color_queries_work_in_both_paths_without_runtime_context() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT RGB(1,2,3) AS Packed,QBColor(4) AS DarkRed,RGB(999,0,0) AS Clamped",
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
                vec!["Packed", "DarkRed", "Clamped"],
                vec!["197121", "128", "255"]
            ]
        );

        for expression in [
            "RGB(Null,1/0,0)",
            "RGB(1,2)",
            "RGB(1,2,3,4)",
            "RGB(-1,0,0)",
            "RGB('1',2,3)",
            "QBColor(16)",
            "QBColor(*)",
            "VBA.RGB(1,2,3)",
            "Round(1,2,3)",
            "IIf(False,QBColor(16),0)",
            "RGB(1,2,3),1/0",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
