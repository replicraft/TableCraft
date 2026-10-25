use std::process::Command;

#[test]
fn both_query_modes_expose_combined_width_script_and_case_transforms() {
    for mode in ["eval", "run"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query", mode,
                "SELECT StrConv('hello WORLD',7,1041) AS W,StrConv('aｶﾞ',37,1041) AS H,StrConv('aが',24,1041) AS N",
                "--format", "tsv",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        assert!(output.stderr.is_empty());
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(output.stdout.as_slice());
        let cells = reader.records().next().unwrap().unwrap();
        assert_eq!(
            cells.iter().collect::<Vec<_>>(),
            ["\"Ｈｅｌｌｏ　Ｗｏｒｌｄ\"", "\"Ａが\"", "\"aｶﾞ\""]
        );
    }
}

#[test]
fn combined_conversion_failures_publish_no_earlier_projection() {
    for mode in ["eval", "run"] {
        for expression in [
            "StrConv(NULL,37,1033)",
            "StrConv('x',100,1041)",
            "StrConv(ChrW(55296),37,1041)",
            "StrConv('x',37,1/0)",
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", mode, &format!("SELECT 1 AS Earlier,{expression}")])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
        }
    }
}
