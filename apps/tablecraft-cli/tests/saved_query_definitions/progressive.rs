use super::*;

fn database(expression: &str, parameter: bool) -> Database {
    let row = |attribute| Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    };
    let mut fragments = vec![
        Fragment {
            name1: Some("Items"),
            ..row(5)
        },
        Fragment {
            expression: Some(expression),
            name1: Some("Chosen"),
            ..row(6)
        },
    ];
    if parameter {
        fragments.push(Fragment {
            name1: Some("Floor"),
            flag: Some(4),
            ..row(2)
        });
        fragments.push(Fragment {
            expression: Some("[Id]>=[Floor]"),
            ..row(8)
        });
    }
    Database::new(fixture::database_with_query_flags(&fragments, 0x80000000))
}

#[test]
fn explicit_saved_streams_use_parameters_and_normal_tsv_publication() {
    let db = database("[Id]", true);
    for batch in ["1", "2"] {
        let result = cli_support::run(&[
            "query",
            "run",
            "--saved",
            "Saved",
            "--database",
            db.path(),
            "--param",
            "Floor=long:2",
            "--stream",
            "--batch-rows",
            batch,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Chosen\n2\n3\n");
    }
}

#[test]
fn saved_named_pipeline_sources_stay_explicit_and_unknown_calls_never_execute() {
    let db = database("[Id]", false);
    let result = cli_support::run(&[
        "query",
        "run",
        "SELECT Q.Chosen FROM Saved Q WHERE Q.Chosen>=2",
        "--database",
        db.path(),
        "--stream",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"Chosen\n2\n3\n");

    let unknown = database("NeverRunMe([Id])", false);
    let refused = cli_support::run(&[
        "query",
        "run",
        "--saved",
        "Saved",
        "--database",
        unknown.path(),
        "--stream",
    ]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(refused.stdout.is_empty());
}
