use super::*;
fn crosstab(flags: i16, transform: &str, pivot: &str) -> Vec<u8> {
    let row = |attribute| Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    };
    fixture::database_with_query_flags(
        &[
            Fragment {
                flag: Some(6),
                ..row(1)
            },
            Fragment {
                flag: Some(flags),
                name1: if flags & 16 != 0 { Some("1") } else { None },
                ..row(3)
            },
            Fragment {
                name1: Some("Items"),
                ..row(5)
            },
            Fragment {
                expression: Some("0"),
                flag: Some(2),
                name1: Some("Bucket"),
                ..row(6)
            },
            Fragment {
                expression: Some(transform),
                flag: Some(0),
                name1: Some("Measure"),
                order: Some(&[0, 0, 0, 2]),
                ..row(6)
            },
            Fragment {
                expression: Some(pivot),
                flag: Some(1),
                order: Some(&[0, 0, 0, 3]),
                ..row(6)
            },
            Fragment {
                expression: Some("0"),
                flag: Some(2),
                ..row(9)
            },
        ],
        0x10,
    )
}
#[test]
fn native_crosstab_printing_and_explicit_execution_share_the_owned_query_path() {
    let db = Database::new(crosstab(0, "Count(*)", "Id IN (3,1,4)"));
    let result = cli_support::run(&["query", "saved", "--database", db.path(), "Saved"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "TRANSFORM Count(*) AS [Measure] SELECT 0 AS [Bucket] FROM [Items] GROUP BY 0 PIVOT [Id] IN (3, 1, 4);\n"
    );
    let result = cli_support::run(&["query", "run", "--database", db.path(), "--saved", "Saved"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "Bucket\t3\t1\t4\n0\t1\t1\tNULL\n"
    );
    // Named crosstab schemas remain unsupported; discovery is not an implicit scan.
    let result = cli_support::run(&[
        "query",
        "run",
        "SELECT * FROM Saved",
        "--database",
        db.path(),
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
#[test]
fn crosstab_owner_functions_and_invalid_fragments_remain_inert_or_buffered_errors() {
    for (flags, transform, pivot, reconstruct) in [
        (4, "Count(*)", "Id", true),
        (0, "NeverRunMe()", "Id", true),
        (0, "Count(*);DELETE FROM Items", "Id", false),
        (0, "Count(*)", "Id ORDER BY 1", false),
    ] {
        let db = Database::new(crosstab(flags, transform, pivot));
        let result = cli_support::run(&["query", "saved", "--database", db.path(), "Saved"]);
        assert_eq!(result.status.success(), reconstruct);
        if !reconstruct {
            assert!(result.stdout.is_empty());
        }
        assert!(
            cli_support::run(&["inspect", db.path(), "--query", "Saved"])
                .status
                .success()
        );
        let result = cli_support::run(&[
            "-vv",
            "query",
            "run",
            "--database",
            db.path(),
            "--saved",
            "Saved",
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        let diagnostics = String::from_utf8_lossy(&result.stderr);
        for private in [db.path(), "Saved", "NeverRunMe", "DELETE FROM Items"] {
            assert!(!diagnostics.contains(private));
        }
    }
}

#[test]
fn stored_crosstab_top_executes_and_omitted_cell_errors_remain_buffered() {
    let db = Database::new(crosstab(16, "Count(*)", "Id IN (3,1,4)"));
    let result = cli_support::run(&["query", "run", "--saved", "Saved", "--database", db.path()]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "Bucket\t3\t1\t4\n0\t1\t1\tNULL\n"
    );
    let result = cli_support::run(&[
        "query",
        "run",
        "TRANSFORM Sum(1/(Id-2)) SELECT TOP 1 Id FROM Items GROUP BY Id ORDER BY Id PIVOT Id",
        "--database",
        db.path(),
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("division by zero"));
}
