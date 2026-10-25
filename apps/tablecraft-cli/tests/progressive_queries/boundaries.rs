use super::*;

#[test]
fn unsupported_relational_and_source_using_shapes_publish_nothing() {
    let file = FileCopy::new(fixture::database());
    for sql in [
        "SELECT Id FROM Items ORDER BY Id",
        "SELECT TOP 1 PERCENT Id FROM Items",
        "SELECT TOP 1 Id FROM Items ORDER BY Id",
        "SELECT DISTINCT Id FROM Items",
        "SELECT Sum(Id) FROM Items",
        "SELECT Count(*) FROM Items HAVING Count(*)>0",
        "SELECT Id FROM Items GROUP BY Id",
        "SELECT Id FROM Items UNION SELECT Id FROM Items",
        "SELECT Id FROM Items UNION ALL SELECT Sum(Id) FROM Items WHERE False",
        "SELECT A.Id FROM Items A INNER JOIN Items B ON A.Id=B.Id",
        "SELECT Q.Id FROM (SELECT Id FROM Items) Q",
        "SELECT (SELECT 1) FROM Items",
        "SELECT DCount('*','Items') FROM Items",
        "SELECT Eval('1+2') FROM Items",
        "SELECT Now() FROM Items WHERE False",
        "SELECT Rnd() FROM Items WHERE False",
    ] {
        assert!(
            failure(file.query(sql, &["--stream", "--batch-rows", "1"])).is_empty(),
            "{sql}"
        );
    }
}

#[test]
fn batch_ranges_and_incompatible_modes_fail_before_missing_source_is_opened() {
    for arguments in [
        vec!["--stream", "--batch-rows", "0"],
        vec!["--stream", "--batch-rows", "1001"],
        vec!["--batch-rows", "1"],
        vec!["--stream", "--saved", "Q"],
        vec!["--stream", "--sqlite", "missing-stream.sqlite"],
        vec!["--stream", "--index", "Items", "Missing"],
        vec!["--stream", "--auto-index"],
        vec!["--stream", "--link", "Items", "missing-link.accdb"],
        vec!["--stream", "--external", "token", "missing-external.accdb"],
        vec!["--stream", "--collections"],
        vec!["--stream", "--multivalues"],
        vec!["--stream", "--stored-calculations"],
        vec!["--stream", "--output-bytes", "0"],
        vec!["--stream", "--output-bytes", "16777217"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                "run",
                "--database",
                "missing-progressive-query-input.accdb",
                "SELECT Id FROM Items",
            ])
            .args(&arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Usage:") || stderr.contains("invalid value"),
            "{arguments:?}: {stderr}"
        );
    }
}

#[test]
fn streaming_requires_native_database_and_refuses_table_rendering_before_io() {
    let missing_database = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "run", "SELECT Id FROM Items", "--stream"])
        .output()
        .unwrap();
    assert_eq!(missing_database.status.code(), Some(2));
    assert!(missing_database.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing_database.stderr).contains("Usage:"));

    let table = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "run",
            "--database",
            "missing-progressive-query-input.accdb",
            "SELECT Id FROM Items",
            "--stream",
            "--format",
            "table",
        ])
        .output()
        .unwrap();
    assert_eq!(table.status.code(), Some(2));
    assert!(table.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&table.stderr)
            .to_ascii_lowercase()
            .contains("tsv")
    );
}
