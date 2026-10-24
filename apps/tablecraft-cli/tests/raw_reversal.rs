mod support;

use support::run;

#[test]
fn retained_reversal_reaches_both_cli_evaluators() {
    for command in ["eval", "run"] {
        let output = run(&[
            "query",
            command,
            "SELECT StrReverse(StrReverse('a🦀中')) AS Repaired,LenB(StrReverse('🦀')) AS Bytes",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(output.stdout.as_slice());
        assert_eq!(
            reader.headers().unwrap(),
            &csv::StringRecord::from(vec!["Repaired", "Bytes"])
        );
        let rows = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get(0), Some("\"a🦀中\""));
        assert_eq!(rows[0].get(1), Some("4"));
    }
}

#[test]
fn invalid_final_reversal_and_late_failures_never_publish_a_prefix() {
    for command in ["eval", "run"] {
        for sql in [
            "SELECT 1 AS Prefix,StrReverse('🦀')",
            "SELECT 1 AS Prefix,LenB(StrReverse(ChrB(1)))",
            "SELECT 1 AS Prefix,IIf(True,StrReverse(ChrW(55296)),1/0)",
        ] {
            let output = run(&["query", command, sql]);
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
        }
    }
}
