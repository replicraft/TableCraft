//! Native saved aggregate definitions compose through the retained stage driver.
use super::*;

const SELECT_FLAGS: u32 = 0x8000_0000;

fn database() -> Database {
    let row = |attribute| Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    };
    let fragments = [
        Fragment {
            name1: Some("Items"),
            ..row(5)
        },
        Fragment {
            name1: Some("K"),
            expression: Some("[Id] Mod 2"),
            ..row(6)
        },
        Fragment {
            name1: Some("N"),
            expression: Some("Count(*)"),
            order: Some(&[0, 0, 0, 2]),
            ..row(6)
        },
        Fragment {
            expression: Some("[Id] Mod 2"),
            ..row(9)
        },
    ];
    Database::new(fixture::database_with_query_flags(&fragments, SELECT_FLAGS))
}

#[test]
fn explicit_saved_grouping_and_named_aggregate_consumers_share_streaming_semantics() {
    let db = database();
    for batch in ["1", "2"] {
        let direct = cli_support::run(&[
            "query",
            "run",
            "--saved",
            "Saved",
            "--database",
            db.path(),
            "--stream",
            "--batch-rows",
            batch,
        ]);
        assert!(
            direct.status.success(),
            "{}",
            String::from_utf8_lossy(&direct.stderr)
        );
        assert_eq!(direct.stdout, b"K\tN\n1\t2\n0\t1\n");

        let sql = "SELECT Sum(N) AS Total,Count(*) AS [Groups] FROM Saved Q";
        for streaming in [false, true] {
            let mut args = vec!["query", "run", sql, "--database", db.path()];
            if streaming {
                args.extend(["--stream", "--batch-rows", batch]);
            }
            let result = cli_support::run(&args);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, b"Total\tGroups\n3\t2\n");
        }
    }
}
