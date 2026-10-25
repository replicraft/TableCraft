#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod agile_fixture;
#[path = "support/mod.rs"]
mod cli_support;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/linked_tables/fixture.rs"]
mod linked_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod target_fixture;

#[path = "external_queries/per_table.rs"]
mod per_table;

use cli_support::run;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const PAGE_BYTES: usize = 4096;
const ITEMS_PAGE: usize = 12;
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Database {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl Database {
    fn new(bytes: Vec<u8>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-external-query-{}-{}.accdb",
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

fn image(profile: u8, score: i32) -> Vec<u8> {
    let mut bytes = target_fixture::database();
    bytes[20] = profile;
    if profile != 1 {
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    }
    let rows = (1_i32..=3)
        .map(|id| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend_from_slice(&id.to_le_bytes());
            row.extend_from_slice(&(id * score).to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    let page = support::data_page(10, &rows);
    bytes[ITEMS_PAGE * PAGE_BYTES..(ITEMS_PAGE + 1) * PAGE_BYTES].copy_from_slice(&page);
    bytes
}

#[test]
fn exact_tokens_select_explicit_targets_across_priority_profiles() {
    for profile in [1, 2, 5, 6] {
        let root = Database::new(image(profile, 10));
        let target = Database::new(image(profile, 100));
        let output = run(&[
            "query",
            "run",
            "SELECT Id,Score FROM Items IN 'opaque-token' WHERE Id >= 2 ORDER BY Id",
            "--database",
            root.path(),
            "--external",
            "opaque-token",
            target.path(),
            "--auto-index",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, b"Id\tScore\n2\t200\n3\t300\n");
    }
}

#[test]
fn nested_selects_retain_their_own_explicit_source_scope() {
    let root = Database::new(image(6, 10));
    let target = Database::new(image(6, 100));
    let output = run(&[
        "query",
        "run",
        "SELECT x.Id,x.Score,(SELECT Sum(Score) FROM Items) AS LocalTotal FROM Items AS x IN 'remote' ORDER BY x.Id",
        "--database",
        root.path(),
        "--external",
        "remote",
        target.path(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"Id\tScore\tLocalTotal\n1\t100\t60\n2\t200\t60\n3\t300\t60\n"
    );

    let output = run(&[
        "query",
        "run",
        "SELECT Id,(SELECT Sum(Score) FROM Items IN 'remote') AS RemoteTotal FROM Items ORDER BY Id",
        "--database",
        root.path(),
        "--external",
        "remote",
        target.path(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Id\tRemoteTotal\n1\t600\n2\t600\n3\t600\n");
}

#[test]
fn linked_root_and_external_target_compose_without_namespace_collisions() {
    let root = Database::new(linked_fixture::database(
        6,
        Some("never-opened"),
        Some("Items"),
        None,
    ));
    let linked = Database::new(image(6, 10));
    let external = Database::new(image(6, 100));
    let output = run(&[
        "query",
        "run",
        "SELECT l.Id,l.Score+r.Score AS Total FROM Linked AS l INNER JOIN (SELECT Id,Score FROM Items IN 'remote') AS r ON l.Id=r.Id ORDER BY l.Id",
        "--database",
        root.path(),
        "--link",
        "Linked",
        linked.path(),
        "--external",
        "remote",
        external.path(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Id\tTotal\n1\t110\n2\t220\n3\t330\n");
}

#[test]
fn unbound_mismatched_duplicate_and_provider_tokens_publish_nothing() {
    let root = Database::new(image(6, 10));
    let target = Database::new(image(6, 100));
    let cases = [
        vec![
            "SELECT Id FROM Items IN 'remote' 'Excel 8.0;'",
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
        ],
        vec![
            "SELECT Id FROM Items IN 'remote' [MS Access;]",
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
        ],
        vec![
            "SELECT Id FROM Items IN 'remote'",
            "--database",
            root.path(),
        ],
        vec![
            "SELECT Id FROM Items IN 'REMOTE'",
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
        ],
        vec![
            "SELECT Id FROM Items IN 'remote'",
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
            "--external",
            "remote",
            target.path(),
        ],
        vec![
            "SELECT Id FROM Items IN 'remote'",
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
            "--external-index",
            "missing",
            "Items",
            "Missing",
        ],
        vec![
            "SELECT Id FROM Linked IN 'remote'",
            "--database",
            target.path(),
            "--external",
            "remote",
            root.path(),
        ],
    ];
    for arguments in cases {
        let mut command = vec!["query", "run"];
        command.extend(arguments);
        let output = run(&command);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn encrypted_external_targets_use_the_explicit_password() {
    let root = Database::new(image(6, 10));
    let target = Database::new(agile_fixture::encrypted_image(
        &image(6, 100),
        6,
        "public external password",
    ));
    let arguments = [
        "query",
        "run",
        "SELECT Sum(Score) AS Total FROM Items IN 'remote'",
        "--database",
        root.path(),
        "--external",
        "remote",
        target.path(),
    ];
    let mut correct = arguments.to_vec();
    correct.extend(["-p", "public external password"]);
    let output = run(&correct);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Total\n600\n");

    let mut wrong = arguments.to_vec();
    wrong.extend(["-p", "wrong external password"]);
    for arguments in [arguments.to_vec(), wrong] {
        let output = run(&arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .to_ascii_lowercase()
                .contains("password")
        );
    }
}

#[test]
fn external_index_selection_reaches_only_the_named_target_and_never_falls_back() {
    let root = Database::new(image(6, 10));
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
            "SELECT Count(*) AS N FROM ROULETTE_TB IN 'remote'",
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
            "--external-index",
            "remote",
            "ROULETTE_TB",
            index,
        ]);
        assert_eq!(
            output.status.success(),
            succeeds,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if succeeds {
            assert_eq!(output.stdout, b"N\n38\n");
        } else {
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn external_sources_share_native_limits_and_preserve_buffered_failure() {
    let root = Database::new(image(6, 10));
    let target = Database::new(image(6, 100));
    for limits in [
        ["--native-bytes", "1"],
        ["--native-steps", "1"],
        ["--execution-rows", "1"],
        ["--output-bytes", "1"],
    ] {
        let mut arguments = vec![
            "query",
            "run",
            "SELECT Id,Score FROM Items IN 'remote'",
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
        ];
        arguments.extend(limits);
        let output = run(&arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn external_help_and_argument_dependencies_are_explicit() {
    let help = run(&["query", "run", "--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("--external <DATABASE_TOKEN> <PATH>"));
    assert!(help.contains("--external-index <DATABASE_TOKEN> <TABLE> <INDEX>"));
    for arguments in [
        vec![
            "query",
            "run",
            "SELECT 1",
            "--external",
            "remote",
            "missing.accdb",
        ],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--database",
            "missing.accdb",
            "--external-index",
            "remote",
            "Items",
            "Missing",
        ],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--database",
            "missing.accdb",
            "--sqlite",
            "missing.sqlite",
            "--external",
            "remote",
            "missing.accdb",
        ],
    ] {
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[path = "external_queries/progressive.rs"]
mod progressive;
