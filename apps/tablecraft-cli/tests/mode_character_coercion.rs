mod support;
use support::run;

#[test]
fn cli_mode_and_character_slots_share_explicit_numeric_policy() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Asc(Chr('65,5')),AscW(ChrW('65,5')),AscB(ChrB('65,5')),StrComp('10','2','0,5'),InStr(1,'abc','b','0,5'),Replace('banana','an','X','2,5','2,5','0,5')",
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
            ["66", "66", "66", "-1", "2", "\"XXa\""]
        );
        let result = run(&[
            "query",
            mode,
            "SELECT Asc(Chr('128'))",
            "--number-format",
            "en-US",
            "--character-code-page",
            "1252",
        ]);
        assert!(result.status.success());
        for sql in [
            "SELECT 99,Chr('65')",
            "SELECT 99,ChrW(55296)",
            "SELECT StrComp(NULL,1/0,'bad')",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}
