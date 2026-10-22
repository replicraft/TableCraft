mod support;
use support::run;

#[test]
fn integer_operators_and_final_predicates_work_through_cli_commands() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT CByte(128) Or CByte(1),CInt(1) And CInt(2),Not CLng(0),CByte(1) Imp CByte(2),True Xor False",
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
            ["129", "0", "-1", "254", "TRUE"]
        );
        let failure = run(&["query", mode, "SELECT 1,0 And (1/0)"]);
        assert!(!failure.status.success());
        assert!(failure.stdout.is_empty());
    }
    let result = run(&[
        "query",
        "run",
        "SELECT ID FROM (SELECT 1 AS ID UNION ALL SELECT 2 UNION ALL SELECT 3) AS A WHERE ID And 2 ORDER BY ID",
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
    assert_eq!(rows, ["2", "3"]);
}
