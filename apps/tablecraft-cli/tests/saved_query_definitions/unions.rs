use super::*;
fn bytes(first: &str, second: &str, flags: i16) -> Vec<u8> {
    let row = |attribute| Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    };
    fixture::database_with_query_flags(
        &[
            Fragment {
                flag: Some(9),
                ..row(1)
            },
            Fragment {
                flag: Some(flags),
                ..row(3)
            },
            Fragment {
                expression: Some(first),
                name2: Some("X7YZ_____1"),
                ..row(5)
            },
            Fragment {
                expression: Some(second),
                name2: Some("X7YZ_____2"),
                ..row(5)
            },
        ],
        0x80,
    )
}
#[test]
fn saved_union_printing_execution_and_named_resolution_remain_explicit() {
    let db = Database::new(bytes("SELECT 1 AS N", "SELECT 1 UNION ALL SELECT 2", 2));
    let result = cli_support::run(&["query", "saved", "--database", db.path(), "Saved"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "SELECT 1 AS [N] UNION SELECT 1 UNION ALL SELECT 2;\n"
    );
    for args in [
        vec!["query", "run", "--database", db.path(), "--saved", "Saved"],
        vec![
            "query",
            "run",
            "SELECT * FROM Saved",
            "--database",
            db.path(),
        ],
    ] {
        let result = cli_support::run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap(), "N\n1\n2\n");
    }
}
#[test]
fn saved_union_owner_cycles_and_inert_functions_never_execute_implicitly() {
    for (first, flags) in [
        ("SELECT NeverRunMe() AS N", 0),
        ("SELECT 1 AS N", 4),
        ("Saved", 0),
    ] {
        let db = Database::new(bytes(first, "SELECT 2", flags));
        assert!(
            cli_support::run(&["query", "saved", "--database", db.path(), "Saved"])
                .status
                .success()
        );
        assert!(
            cli_support::run(&["inspect", db.path(), "--query", "Saved"])
                .status
                .success()
        );
        for args in [
            vec!["query", "run", "--database", db.path(), "--saved", "Saved"],
            vec![
                "query",
                "run",
                "SELECT * FROM Saved",
                "--database",
                db.path(),
            ],
        ] {
            let result = cli_support::run(&args);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
    for first in [
        "DELETE FROM Items",
        "SELECT 1; DELETE FROM Items",
        "PARAMETERS P LONG; SELECT P",
        "SELECT 1 ORDER BY 1",
    ] {
        let db = Database::new(bytes(first, "SELECT 2", 0));
        for args in [
            vec!["query", "saved", "--database", db.path(), "Saved"],
            vec!["query", "run", "--database", db.path(), "--saved", "Saved"],
        ] {
            let result = cli_support::run(&args);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
        assert!(
            cli_support::run(&["inspect", db.path(), "--query", "Saved"])
                .status
                .success()
        );
    }
}
