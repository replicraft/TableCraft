mod support;
use support::run;

#[test]
fn query_selectors_share_explicit_text_and_intrinsic_date_coercions() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT IIf('0,25',1,0),Switch('False',1,'2,5',2),Choose('2,5',1,2,3),IIf(#1899-12-30#,1,0),IIf(#1899-12-30 06:00:00#,1,0),Choose(#1900-01-01 12:00:00#,1,2,3),IsError(IIf('1',CVErr(2001),0))",
            "--number-format",
            "de-DE",
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
            ["1", "2", "2", "0", "1", "2", "TRUE"]
        );
        for sql in [
            "SELECT IIf('1',1,0)",
            "SELECT Switch('1',1)",
            "SELECT Choose('1',1)",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
        for sql in [
            "SELECT IIf('bad',1,1/0)",
            "SELECT Switch('bad',1,True,1/0)",
            "SELECT Choose('bad',1,1/0)",
            "SELECT IIf(CVErr(0),1,0)",
        ] {
            let result = run(&["query", mode, sql, "--number-format", "en-US"]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}
