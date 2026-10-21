mod support;

use support::run;

#[test]
fn utf16_byte_observations_are_not_utf8_or_ansi_counts() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Len('é中🦀') AS Units,LenB('é中🦀') AS Bytes,AscB('中') AS FirstByte,VarType(LenB('x')) AS L,VarType(AscB('x')) AS A,LenB(Null) AS N",
            "--character-code-page",
            "1252",
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
                vec!["Units", "Bytes", "FirstByte", "L", "A", "N"],
                vec!["4", "8", "45", "3", "2", "NULL"]
            ]
        );

        for expression in [
            "AscB('')",
            "AscB(Null)",
            "LenB(1)",
            "LenB(*)",
            "AscB(,)",
            "LenB('x',1/0)",
            "LenB('x'),1/0",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
