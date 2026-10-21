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
    // Literal fixed headings bind as metadata without a discovery scan.
    let result = cli_support::run(&[
        "query",
        "run",
        "SELECT * FROM Saved",
        "--database",
        db.path(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"Bucket\t3\t1\t4\n0\t1\t1\tNULL\n");
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
fn fixed_named_crosstab_projection_does_not_hide_later_unreturned_cell_errors() {
    let db = Database::new(crosstab(0, "Sum(1/(Id-2))", "Id IN (1,2,3)"));
    let inert = cli_support::run(&["query", "saved", "--database", db.path(), "Saved"]);
    assert!(inert.status.success());
    for sql in [
        "SELECT Bucket FROM Saved",
        "SELECT Bucket FROM Saved WHERE Bucket<0",
        "SELECT TOP 1 Bucket FROM Saved",
    ] {
        let result = cli_support::run(&["query", "run", sql, "--database", db.path()]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn named_crosstabs_keep_fixed_headers_through_joins_and_inner_top() {
    let db = Database::new(crosstab(16, "Count(*)", "Id IN (3,1,4)"));
    for sql in [
        "SELECT s.[3],s.[4] FROM Saved AS s",
        "SELECT a.[3],b.[4] FROM Saved AS a INNER JOIN Saved AS b ON a.Bucket=b.Bucket",
    ] {
        let result = cli_support::run(&["query", "run", sql, "--database", db.path()]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"3\t4\n1\tNULL\n");
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
