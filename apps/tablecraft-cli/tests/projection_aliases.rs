mod support;

use std::{fs, path::PathBuf};
use support::run;

#[test]
fn written_aliases_compose_through_the_query_command() {
    let sql = concat!(
        "SELECT CInt(1)+CInt(2) AS FirstValue, ",
        "CCur(FirstValue) AS MoneyValue, VarType(MoneyValue) AS Kind",
    );
    let result = run(&["query", "run", sql, "--declared-arithmetic"]);

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"FirstValue\tMoneyValue\tKind\n3\t3.0000\t6\n"
    );
}

#[test]
fn native_correlated_aliases_retain_nulls_and_original_database_bytes() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let original = fs::read(&path).unwrap();
    let sql = concat!(
        "SELECT r.ORDR_NBR, ",
        "(SELECT Max(p.ORDR_NBR) FROM ROULETTE_TB p ",
        "WHERE p.ORDR_NBR < r.ORDR_NBR) AS PreviousOrder, ",
        "r.ORDR_NBR-PreviousOrder AS Gap FROM ROULETTE_TB r ",
        "WHERE r.ORDR_NBR<=3 ORDER BY r.ORDR_NBR",
    );
    let result = run(&["query", "run", sql, "--database", path.to_str().unwrap()]);

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"ORDR_NBR\tPreviousOrder\tGap\n1\tNULL\tNULL\n2\t1\t1\n3\t2\t1\n"
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn alias_errors_keep_buffered_output_empty() {
    for sql in [
        "SELECT LaterValue AS EarlierValue, 1 AS LaterValue",
        "SELECT LoopValue+1 AS LoopValue",
        "SELECT 1 AS HiddenValue WHERE HiddenValue=1",
        "SELECT 1 AS HiddenValue, (SELECT HiddenValue) AS NestedValue",
        "SELECT 1 AS PriorValue, 1/(PriorValue-1) AS Failure",
    ] {
        let result = run(&["query", "run", sql]);

        assert_eq!(result.status.code(), Some(2), "{sql}");
        assert!(result.stdout.is_empty(), "{sql}");
        assert!(!result.stderr.is_empty(), "{sql}");
    }
}
