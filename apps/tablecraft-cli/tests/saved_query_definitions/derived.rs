use super::*;

fn saved(text: &str) -> Vec<u8> {
    let row = |attribute| Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    };
    fixture::database_with_query_flags(
        &[
            Fragment {
                expression: Some(text),
                name2: Some("d"),
                ..row(5)
            },
            Fragment {
                expression: Some("d.Id"),
                name1: Some("Chosen"),
                ..row(6)
            },
            Fragment {
                expression: Some("Chosen"),
                name1: Some("A"),
                ..row(11)
            },
        ],
        0,
    )
}

#[test]
fn saved_derived_sources_execute_explicitly_and_never_publish_partial_rows_or_run_inert_calls() {
    for (text, accepted) in [
        ("SELECT Id FROM Items WHERE Id>1", true),
        ("SELECT 1/(3-Id) AS Id FROM Items", false),
        ("SELECT NeverRunMe(Id) AS Id FROM Items", false),
    ] {
        let db = Database::new(saved(text));
        let inspect = cli_support::run(&["query", "saved", "--database", db.path(), "Saved"]);
        assert!(
            inspect.status.success(),
            "{}",
            String::from_utf8_lossy(&inspect.stderr)
        );
        for query in [vec!["--saved", "Saved"], vec!["SELECT * FROM Saved"]] {
            let mut args = vec!["query", "run"];
            args.extend(query);
            args.extend(["--database", db.path()]);
            let result = cli_support::run(&args);
            if accepted {
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert_eq!(result.stdout, b"Chosen\n2\n3\n");
            } else {
                assert_eq!(result.status.code(), Some(2));
                assert!(result.stdout.is_empty());
            }
        }
    }
}
