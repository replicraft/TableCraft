mod support;
use support::run;

#[test]
fn repetition_error_data_keeps_null_results_and_atomic_operational_failures() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT String(NULL,CVErr(11)) AS A,String(CVErr(11),NULL) AS B,String(2,CStr(CVErr(11))) AS C",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            table(&result.stdout),
            [vec!["A", "B", "C"], vec!["NULL", "NULL", "\"EE\""]]
        );

        for sql in [
            "SELECT 1,Space(CVErr(11))",
            "SELECT 1,String(-1,CVErr(11))",
            "SELECT 1,String(0,CVErr(11))",
            "SELECT 1,String(CVErr(11),1/0)",
            "SELECT 1,String(NULL,CStr(NULL))",
        ] {
            let result = run(&["query", command, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty(), "{sql}");
        }
    }
}

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
fn construction_and_reversal_share_buffered_scalar_and_query_output() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT Space(3) AS A,String(5,42) AS B,String(3,'éx') AS C,StrReverse('é中') AS D,Space(NULL) AS N,String(2,NULL) AS M",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            table(&result.stdout),
            [
                vec!["A", "B", "C", "D", "N", "M"],
                vec!["\"   \"", "\"*****\"", "\"ééé\"", "\"中́e\"", "NULL", "NULL"]
            ]
        );
        for expression in [
            "Space(-1)",
            "String(NULL,1/0)",
            "String(1,128)",
            "String(1,'😀')",
            "StrReverse('😀')",
            "StrReverse(NULL)",
            "Space(2147483647)",
        ] {
            let result = run(&["query", command, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
