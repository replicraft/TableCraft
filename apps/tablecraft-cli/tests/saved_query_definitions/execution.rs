use super::*;

fn saved(expression: &str, parameter: bool) -> Vec<u8> {
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
        Fragment {
            expression: Some("[Score]"),
            name1: Some("D"),
            ..row(11)
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
    fixture::database_with_query_flags(&fragments, 0x80000000)
}
#[test]
fn explicit_saved_execution_uses_parameters_and_existing_presentation() {
    let db = Database::new(saved("[Id]", true));
    for prefix in [vec![], vec!["-vv"]] {
        let mut args = prefix;
        args.extend([
            "query",
            "run",
            "--saved",
            "Saved",
            "--database",
            db.path(),
            "--param",
            "Floor=long:2",
        ]);
        let result = cli_support::run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Chosen\n3\n2\n");
        let diagnostics = String::from_utf8(result.stderr).unwrap();
        for payload in [db.path(), "Saved", "Floor", "Chosen", "Items"] {
            assert!(!diagnostics.contains(payload));
        }
    }
    let bordered = cli_support::run(&[
        "--format",
        "table",
        "query",
        "run",
        "--saved",
        "Saved",
        "--database",
        db.path(),
        "--param",
        "Floor=long:2",
    ]);
    assert!(bordered.status.success());
    assert!(String::from_utf8(bordered.stdout).unwrap().starts_with('┌'));
    let missing_parameter =
        cli_support::run(&["query", "run", "--saved", "Saved", "--database", db.path()]);
    assert_eq!(missing_parameter.status.code(), Some(2));
    assert!(missing_parameter.stdout.is_empty());
}
#[test]
fn saved_execution_requires_explicit_exclusive_native_input_and_never_runs_unknown_calls() {
    let db = Database::new(saved("NeverRunMe([Id])", false));
    for args in [
        vec!["query", "run", "--saved", "Saved"],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--saved",
            "Saved",
            "--database",
            db.path(),
        ],
        vec!["query", "run", "--saved", "Saved", "--sqlite", db.path()],
        vec!["query", "run", "--saved", "Saved", "--database", db.path()],
        vec![
            "query",
            "run",
            "--saved",
            "Missing",
            "--database",
            db.path(),
        ],
    ] {
        let result = cli_support::run(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(result.stdout.is_empty());
    }
    let reconstruction = cli_support::run(&["query", "saved", "Saved", "--database", db.path()]);
    assert!(reconstruction.status.success());
    assert!(
        String::from_utf8(reconstruction.stdout)
            .unwrap()
            .contains("NeverRunMe")
    );
}
