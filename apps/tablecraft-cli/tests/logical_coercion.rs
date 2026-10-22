mod support;
use support::run;

#[test]
fn contextual_logical_operators_work_in_both_cli_modes_and_buffer_errors() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Not 2.5,CCur(2.5) And CByte(3),Not '2,5','9007199254740993' Or CLngLng(0),Not CDate(-1.25)",
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
            ["-3", "2", "-3", "9007199254740993", "0"]
        );
        for sql in ["SELECT 1,Not '2.5'", "SELECT 1,'1' And 0"] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
        for sql in [
            "SELECT 1,Not 'bad'",
            "SELECT 'bad' And (1/0)",
            "SELECT CByte(0) And CDec(2147483647.5)",
        ] {
            let result = run(&["query", mode, sql, "--number-format", "en-US"]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
    let result = run(&[
        "query",
        "run",
        "SELECT 99 WHERE '0,25'",
        "--number-format",
        "de-DE",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let rows = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(result.stdout.as_slice())
        .records()
        .map(|row| row.unwrap().get(0).unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(rows, ["99"]);
}
