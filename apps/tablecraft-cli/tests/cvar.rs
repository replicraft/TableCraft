mod support;

use support::run;

#[test]
fn cvar_retains_null_text_and_numeric_subtypes_in_both_query_paths() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT CVar(Null) AS N,CVar('001') AS T,VarType(CVar(CByte(255))) AS B,VarType(CVar(CInt(-1))) AS I,VarType(CVar(CSng(1.5))) AS S,VarType(CVar(#2026-10-21#)) AS D",
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
                vec!["N", "T", "B", "I", "S", "D"],
                vec!["NULL", "\"001\"", "17", "2", "4", "7"]
            ]
        );

        for expression in [
            "CVar()",
            "CVar(*)",
            "CVar(,)",
            "CVar(1,1/0)",
            "VBA.CVar(1)",
            "CVar(Array(1,2))",
            "CVar(1/0)",
            "IIf(False,CVar(1/0),0)",
            "CVar(1),1/0",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
