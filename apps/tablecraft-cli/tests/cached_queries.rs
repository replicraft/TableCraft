#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[path = "support/mod.rs"]
mod cli_support;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;

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
            "tablecraft-cached-query-{}-{}.accdb",
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
fn queries_require_the_cached_flag_and_preserve_stored_values_without_recalculation() {
    for code in [5, 6] {
        let mut bytes = calculated_fixture::database(&[Some(999), None, Some(41)]);
        bytes[20] = code;
        let database = Database::new(bytes);
        let output = run(&[
            "query",
            "run",
            "SELECT Id,Score FROM Items",
            "--database",
            database.path(),
            "--stored-calculations",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"Id\tScore\n1\t999\n2\tNULL\n3\t41\n");
        assert!(output.stderr.is_empty());
        let output = run(&[
            "query",
            "run",
            "SELECT Id FROM Items WHERE Score>100",
            "--database",
            database.path(),
            "--stored-calculations",
            "--auto-index",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"Id\n1\n");
        for options in [
            vec![],
            vec!["--stored-calculations", "--index", "Items", "Missing"],
        ] {
            let mut args = vec![
                "query",
                "run",
                "SELECT Score FROM Items",
                "--database",
                database.path(),
            ];
            args.extend(options);
            let output = run(&args);
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn cached_query_cli_buffers_late_errors_and_keeps_ordinary_projections_available() {
    let mut bytes = calculated_fixture::database(&[Some(20), Some(41)]);
    let second =
        u16::from_le_bytes(bytes[12 * 4096 + 16..12 * 4096 + 18].try_into().unwrap()) as usize;
    bytes[12 * 4096 + second + 6] = 1;
    let database = Database::new(bytes);
    for sql in [
        "SELECT TOP 1 Score FROM Items",
        "SELECT Score FROM Items WHERE Id=999",
    ] {
        let output = run(&[
            "query",
            "run",
            sql,
            "--database",
            database.path(),
            "--stored-calculations",
        ]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let output = run(&[
        "query",
        "run",
        "SELECT Id FROM Items",
        "--database",
        database.path(),
        "--stored-calculations",
    ]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Id\n1\n2\n");
    for args in [
        vec!["query", "run", "SELECT 1", "--stored-calculations"],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--sqlite",
            "missing.sqlite",
            "--stored-calculations",
        ],
    ] {
        let output = run(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}
