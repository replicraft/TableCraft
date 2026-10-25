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

fn image(profile: u8, factor: i32, projection: &str, redirect: bool) -> Vec<u8> {
    let mut fragments = vec![
        Fragment {
            name1: Some(if redirect { "Saved" } else { "Items" }),
            ..row(5)
        },
        Fragment {
            expression: Some(projection),
            name1: Some("Chosen"),
            ..row(6)
        },
    ];
    if redirect {
        fragments.push(Fragment {
            name1: Some("remote"),
            flag: Some(2),
            ..row(4)
        });
    }
    let mut bytes = fixture::database_with_query_flags(&fragments, 0x8000_0000);
    let rows = (1_i32..=3)
        .map(|id| {
            let mut record = 2_u16.to_le_bytes().to_vec();
            record.extend_from_slice(&id.to_le_bytes());
            record.extend_from_slice(&(id * factor).to_le_bytes());
            record.push(3);
            (0, record)
        })
        .collect::<Vec<_>>();
    bytes[ITEMS_DATA_PAGE * PAGE_BYTES..(ITEMS_DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(10, &rows));
    bytes[20] = profile;
    if profile == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes
}

#[test]
fn root_and_external_saved_names_keep_their_own_sources_across_profiles() {
    for profile in [1, 2, 5, 6] {
        let root = Database::new(image(profile, 10, "Score", false));
        let target = Database::new(image(profile, 100, "Score", false));
        for (sql, expected) in [
            (
                "SELECT Chosen FROM Saved ORDER BY Chosen",
                &b"Chosen\n10\n20\n30\n"[..],
            ),
            (
                "SELECT Chosen FROM Saved IN 'remote' ORDER BY Chosen",
                &b"Chosen\n100\n200\n300\n"[..],
            ),
        ] {
            let result = cli_support::run(&[
                "query",
                "run",
                sql,
                "--database",
                root.path(),
                "--external",
                "remote",
                target.path(),
            ]);
            assert!(result.status.success(), "{:?}", result);
            assert_eq!(result.stdout, expected);
            assert!(result.stderr.is_empty());
        }
    }
}

#[test]
fn stored_external_dependency_with_the_same_name_is_not_a_local_cycle() {
    let root = Database::new(image(6, 10, "Chosen", true));
    let target = Database::new(image(6, 100, "Score", false));
    for input in [vec!["--saved", "Saved"], vec!["SELECT Chosen FROM Saved"]] {
        let mut arguments = vec!["query", "run"];
        arguments.extend(input);
        arguments.extend([
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
        ]);
        let result = cli_support::run(&arguments);
        assert!(result.status.success(), "{:?}", result);
        assert_eq!(result.stdout, b"Chosen\n100\n200\n300\n");
    }
}

#[test]
fn target_definition_failures_stay_buffered_and_never_use_the_root_definition() {
    let root = Database::new(image(6, 10, "Score", false));
    for projection in ["NeverRunMe(Score)", "Score/(Id-3)"] {
        let target = Database::new(image(6, 100, projection, false));
        let inert = cli_support::run(&["query", "saved", "--database", target.path(), "Saved"]);
        assert!(inert.status.success(), "{:?}", inert);

        let failed = cli_support::run(&[
            "query",
            "run",
            "SELECT Chosen FROM Saved IN 'remote'",
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
        ]);
        assert_eq!(failed.status.code(), Some(2), "{:?}", failed);
        assert!(failed.stdout.is_empty());

        let local = cli_support::run(&[
            "query",
            "run",
            "SELECT Chosen FROM Saved",
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
        ]);
        assert!(local.status.success(), "{:?}", local);
        assert_eq!(local.stdout, b"Chosen\n10\n20\n30\n");
    }
}
