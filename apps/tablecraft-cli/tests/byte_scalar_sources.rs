#[path = "support/mod.rs"]
mod cli_support;
use cli_support::run;

#[test]
fn scalar_byte_functions_work_in_both_query_commands_with_explicit_formats() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT LeftB(1234,4) AS L,MidB(12.5,5,2) AS M,InStrB(1,1234,23,0) AS P,StrConv(True,1,1033) AS S",
            "--number-format",
            "de-de",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice());
        assert_eq!(
            reader.headers().unwrap(),
            &csv::StringRecord::from(vec!["L", "M", "P", "S"])
        );
        assert_eq!(
            reader.records().next().unwrap().unwrap(),
            csv::StringRecord::from(vec!["\"12\"", "\",\"", "3", "\"TRUE\""])
        );
        assert!(reader.records().next().is_none());
        for sql in ["SELECT 1,LeftB(1234,1)", "SELECT 1,StrConv(1234,1,1/0)"] {
            let result = run(&["query", mode, sql, "--number-format", "en-us"]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
        let result = run(&["query", mode, "SELECT LeftB(1234,2)"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}
