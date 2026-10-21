use super::*;

#[test]
fn stored_bracketed_names_bind_parameters_and_publish_literal_headers() {
    let row = |attribute| Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    };
    let db = Database::new(fixture::database_with_query_flags(
        &[
            Fragment {
                name1: Some("[Enter a minimum]"),
                flag: Some(4),
                ..row(2)
            },
            Fragment {
                name1: Some("[Items]"),
                name2: Some("[Select]"),
                ..row(5)
            },
            Fragment {
                name1: Some("[Chosen Key]"),
                expression: Some("[Select].[Id]"),
                ..row(6)
            },
            Fragment {
                expression: Some("[Select].[Id]>=[Enter a minimum]"),
                ..row(8)
            },
        ],
        0,
    ));
    for query in [vec!["--saved", "Saved"], vec!["SELECT * FROM Saved"]] {
        let mut args = vec!["query", "run"];
        args.extend(query);
        args.extend(["--database", db.path(), "--param", "Enter a minimum=long:2"]);
        let result = cli_support::run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Chosen Key\n2\n3\n");
    }
    let result = cli_support::run(&["query", "run", "--saved", "Saved", "--database", db.path()]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
