use super::*;

const PAGE_BYTES: usize = 4096;
const ITEMS_DATA_PAGE: usize = 12;

fn row(attribute: u8) -> Fragment<'static> {
    Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    }
}

fn database(profile: u8, expression: &str, connection: Option<&str>) -> Vec<u8> {
    let mut bytes = fixture::database_with_query_flags(
        &[
            Fragment {
                name1: Some("remote"),
                expression: connection,
                flag: Some(2),
                ..row(4)
            },
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
                expression: Some("Id"),
                ..row(11)
            },
        ],
        0x8000_0000,
    );
    bytes[20] = profile;
    if profile == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes
}

fn target(profile: u8) -> Vec<u8> {
    let mut bytes = database(profile, "Score", None);
    let rows = (1_i32..=3)
        .map(|id| {
            let mut record = 2_u16.to_le_bytes().to_vec();
            record.extend_from_slice(&id.to_le_bytes());
            record.extend_from_slice(&(id * 100).to_le_bytes());
            record.push(3);
            (0, record)
        })
        .collect::<Vec<_>>();
    let page = support::data_page(10, &rows);
    bytes[ITEMS_DATA_PAGE * PAGE_BYTES..(ITEMS_DATA_PAGE + 1) * PAGE_BYTES].copy_from_slice(&page);
    bytes
}

#[test]
fn saved_and_named_external_selects_use_the_explicit_target_across_priority_profiles() {
    for profile in [1, 2, 5, 6] {
        let root = Database::new(database(profile, "Score", None));
        let remote = Database::new(target(profile));
        let inert = cli_support::run(&["query", "saved", "--database", root.path(), "Saved"]);
        assert!(inert.status.success(), "{:?}", inert);
        assert_eq!(
            inert.stdout,
            b"SELECT [Score] AS [Chosen] FROM [Items] IN 'remote' ORDER BY [Id] ASC;\n"
        );

        for input in [
            vec!["--saved", "Saved"],
            vec!["SELECT Chosen FROM Saved ORDER BY Chosen"],
        ] {
            let mut args = vec!["query", "run"];
            args.extend(input);
            args.extend([
                "--database",
                root.path(),
                "--external",
                "remote",
                remote.path(),
            ]);
            let result = cli_support::run(&args);
            assert!(result.status.success(), "{:?}", result);
            assert_eq!(result.stdout, b"Chosen\n100\n200\n300\n");
            assert!(result.stderr.is_empty());
        }
    }
}

#[test]
fn saved_external_metadata_never_grants_an_implicit_open_or_provider_connection() {
    let remote = Database::new(target(6));
    for connection in [None, Some(""), Some("ODBC;DATABASE=private")] {
        let root = Database::new(database(6, "Score", connection));
        let inert = cli_support::run(&["query", "saved", "--database", root.path(), "Saved"]);
        assert!(inert.status.success(), "{:?}", inert);

        for mapping in [vec![], vec!["--external", "Remote", remote.path()]] {
            let mut args = vec![
                "-vv",
                "query",
                "run",
                "--saved",
                "Saved",
                "--database",
                root.path(),
            ];
            args.extend(mapping);
            let result = cli_support::run(&args);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            let diagnostics = String::from_utf8_lossy(&result.stderr);
            for private in [root.path(), remote.path(), "ODBC;DATABASE=private"] {
                assert!(!diagnostics.contains(private));
            }
        }

        if connection.is_some() {
            let result = cli_support::run(&[
                "query",
                "run",
                "--saved",
                "Saved",
                "--database",
                root.path(),
                "--external",
                "remote",
                remote.path(),
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn saved_external_late_errors_and_exhausted_allowances_publish_no_rows() {
    let remote = Database::new(target(6));
    for (expression, extra) in [
        ("100/(Id-3)", vec![]),
        ("Score", vec!["--execution-steps", "1"]),
        ("Score", vec!["--native-steps", "1"]),
        ("Score", vec!["--output-bytes", "1"]),
    ] {
        let root = Database::new(database(6, expression, None));
        let mut args = vec![
            "query",
            "run",
            "--saved",
            "Saved",
            "--database",
            root.path(),
            "--external",
            "remote",
            remote.path(),
        ];
        args.extend(extra);
        let result = cli_support::run(&args);
        assert_eq!(result.status.code(), Some(2), "{:?}", result);
        assert!(result.stdout.is_empty());
    }
}
