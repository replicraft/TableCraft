#[path = "support/mod.rs"]
mod cli_support;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/linked_tables/fixture.rs"]
mod linked_fixture;
#[path = "linked_queries/options.rs"]
mod options;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod target_fixture;

use cli_support::run;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Database {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl Database {
    fn new(bytes: Vec<u8>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-linked-query-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
        Self { path, bytes }
    }

    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        assert_eq!(fs::read(&self.path).unwrap(), self.bytes);
        fs::remove_file(&self.path).unwrap();
    }
}

#[test]
fn explicit_paths_route_priority_profiles_without_opening_stored_paths() {
    for code in [1, 2, 5, 6] {
        let mut root = linked_fixture::database(
            6,
            Some("/this/stored/path/is/not/opened.accdb"),
            Some("items"),
            None,
        );
        root[20] = code;
        if code == 1 {
            root[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let root = Database::new(root);
        let mut target = target_fixture::database();
        target[20] = code;
        if code != 1 {
            target[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let target = Database::new(target);
        let output = run(&[
            "query",
            "run",
            "SELECT Id, Score FROM Linked WHERE Id >= 2 ORDER BY Id",
            "--database",
            root.path(),
            "--link",
            "linked",
            target.path(),
            "--auto-index",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, b"Id\tScore\n2\t20\n3\t30\n");
    }
}

#[test]
fn links_remain_inert_without_mapping_and_fail_without_publishing_rows() {
    let root = Database::new(linked_fixture::database(
        6,
        Some("missing"),
        Some("Items"),
        None,
    ));
    let output = run(&[
        "query",
        "run",
        "SELECT 1 AS Answer",
        "--database",
        root.path(),
    ]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Answer\n1\n");

    let output = run(&[
        "query",
        "run",
        "SELECT * FROM Linked",
        "--database",
        root.path(),
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn invalid_explicit_mappings_and_provider_links_publish_nothing() {
    let root = Database::new(linked_fixture::database(
        6,
        Some("missing"),
        Some("Items"),
        None,
    ));
    let target = Database::new(target_fixture::database());
    for extra in [
        vec!["--link", "Unknown", target.path()],
        vec![
            "--link",
            "Linked",
            target.path(),
            "--link",
            "Linked",
            target.path(),
        ],
        vec!["--link", "Linked", root.path()],
        vec!["--link", "Linked", target.path(), "--native-bytes", "1"],
    ] {
        let mut arguments = vec![
            "query",
            "run",
            "SELECT TOP 1 * FROM Linked",
            "--database",
            root.path(),
        ];
        arguments.extend(extra);
        let output = run(&arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }

    let provider = Database::new(linked_fixture::database(
        4,
        Some("missing"),
        Some("Items"),
        Some("ODBC;"),
    ));
    let output = run(&[
        "query",
        "run",
        "SELECT * FROM Linked",
        "--database",
        provider.path(),
        "--link",
        "Linked",
        "/not/an/authorized/provider.accdb",
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("No such file"));
}

#[test]
fn link_help_and_argument_dependencies_are_explicit() {
    let output = run(&["query", "run", "--help"]);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("--link <ALIAS> <PATH>")
    );

    for arguments in [
        vec![
            "query",
            "run",
            "SELECT 1",
            "--link",
            "Alias",
            "target.accdb",
        ],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--database",
            "root.accdb",
            "--link",
            "Alias",
        ],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--sqlite",
            "db.sqlite",
            "--link",
            "Alias",
            "target.accdb",
        ],
    ] {
        let output = run(&arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn selected_link_indexes_reach_target_and_never_silently_fall_back() {
    let root = Database::new(linked_fixture::database(
        6,
        Some("opaque-index-target"),
        Some("ROULETTE_TB"),
        None,
    ));
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let target = Database::new(fs::read(target).unwrap());
    for (index, succeeds) in [
        ("ORDR_NBR", true),
        ("MissingIndex", false),
        ("Number", true),
    ] {
        let output = run(&[
            "query",
            "run",
            "SELECT Count(*) AS N FROM Linked",
            "--database",
            root.path(),
            "--link",
            "Linked",
            target.path(),
            "--index",
            "linked",
            index,
        ]);
        assert_eq!(
            output.status.success(),
            succeeds,
            "index {index}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if succeeds {
            assert_eq!(output.stdout, b"N\n38\n");
        } else {
            assert!(output.stdout.is_empty());
        }
    }
}
