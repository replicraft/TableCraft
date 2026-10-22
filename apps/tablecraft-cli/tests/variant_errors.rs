mod support;

use support::run;

#[test]
fn error_data_has_explicit_query_consumers_and_is_never_a_null_result_cell() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT IsError(CVErr(2001)),TypeName(CVar(CVErr(2001))),VarType(CVErr(2001)),CLng(CVErr(65535)),IIf(False,CVErr(1),7)",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            ["TRUE", "\"Error\"", "10", "65535", "7"]
        );
        let result = run(&[
            "query",
            mode,
            "SELECT CVErr(2001),Null,2001,'#ERROR(2001)',IIf(True,CVErr(0),0)",
        ]);
        assert!(result.status.success());
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            [
                "#ERROR(2001)",
                "NULL",
                "2001",
                "\"#ERROR(2001)\"",
                "#ERROR(0)"
            ]
        );
        for expression in [
            "CVErr(2001),1/0",
            "IsError(1/0)",
            "CVErr(2001)+1",
            "IsError(IIf(True,CVErr(1),1/0))",
            "CInt(CVErr(65535))",
            "CVErr(*)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
