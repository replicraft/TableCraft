use super::*;

fn saved(on: &str) -> Vec<u8> {
    let row = |attribute, ordinal: u8| Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(match ordinal {
            1 => &[0, 0, 0, 1],
            2 => &[0, 0, 0, 2],
            _ => &[0, 0, 0, 3],
        }),
        ..Default::default()
    };
    let mut rows = Vec::new();
    for alias in ["a", "b", "c"] {
        rows.push(Fragment {
            name1: Some("Items"),
            name2: Some(alias),
            ..row(5, 1)
        });
    }
    for (ordinal, expression, name) in [
        (1, "a.Id", "KeyA"),
        (2, "b.Id", "KeyB"),
        (3, "c.Id", "KeyC"),
    ] {
        rows.push(Fragment {
            expression: Some(expression),
            name1: Some(name),
            ..row(6, ordinal)
        });
    }
    for (ordinal, left, right, expression) in [
        (1, "a", "b", "a.Id=b.Id"),
        (2, "b", "c", "b.Id=c.Id"),
        (3, "b", "a", on),
    ] {
        rows.push(Fragment {
            flag: Some(2),
            name1: Some(left),
            name2: Some(right),
            expression: Some(expression),
            ..row(7, ordinal)
        });
    }
    fixture::database_with_query_flags(&rows, 0)
}

#[test]
fn saved_forests_run_explicitly_and_as_named_sources_with_buffered_failures() {
    let db = Database::new(saved("b.Id<>2"));
    for query in [vec!["--saved", "Saved"], vec!["SELECT * FROM Saved"]] {
        let mut args = vec!["query", "run"];
        args.extend(query);
        args.extend(["--database", db.path()]);
        let result = cli_support::run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"KeyA\tKeyB\tKeyC\n1\t1\t1\n2\tNULL\tNULL\n3\t3\t3\n"
        );
    }
    let db = Database::new(saved("NeverRunMe(b.Id)"));
    let inspect = cli_support::run(&["query", "saved", "--database", db.path(), "Saved"]);
    assert!(inspect.status.success());
    for query in [vec!["--saved", "Saved"], vec!["SELECT * FROM Saved"]] {
        let mut args = vec!["query", "run"];
        args.extend(query);
        args.extend(["--database", db.path()]);
        let result = cli_support::run(&args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}
