mod support;
use support::run;

#[test]
fn cli_strconv_options_share_checked_numeric_profiles() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT StrConv('ABC','1,5','1033'),AscB(StrConv('é','129','1033')),StrConv('ABC',CDate(1.5),CDate(1033))",
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
            ["\"abc\"", "201", "\"abc\""]
        );
        for sql in [
            "SELECT 99,StrConv('ABC','1','1033')",
            "SELECT StrConv(NULL,'bad',1/0)",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}
