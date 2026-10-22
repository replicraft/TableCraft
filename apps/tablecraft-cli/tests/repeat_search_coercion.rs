mod support;
use support::run;

#[test]
fn cli_repetition_and_search_convert_only_the_count_and_start_slots() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT String('2,5','65'),Len(Space('2,5')),InStr('2,5','ababab','ab',0),InStrRev('ababab','ab','4,5',0)",
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
        assert_eq!(row.iter().collect::<Vec<_>>(), ["\"66\"", "2", "3", "3"]);
        for sql in [
            "SELECT 99,String('bad',1/0)",
            "SELECT 99,InStr('1','abc','b',0)",
            "SELECT 99,String(2,'🦀')",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}
