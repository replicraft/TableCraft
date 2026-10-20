#[path = "support/mod.rs"]
mod cli_support;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/large_maps/fixture.rs"]
mod map_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
use cli_support::run;
use std::{
    fs,
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
            "tablecraft-native-budget-{}-{}.mdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, &bytes).unwrap();
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
fn explicit_native_bytes_enable_large_queries_in_all_priority_profiles() {
    for code in [1, 2, 5, 6] {
        let mut bytes = map_fixture::database(6000, true, false);
        bytes[20] = code;
        if code != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let db = Database::new(bytes);
        let sql = "SELECT TOP 3 Id FROM Items ORDER BY Id DESC";
        let default = run(&["query", "run", sql, "--database", db.path()]);
        assert_eq!(default.status.code(), Some(2));
        assert!(default.stdout.is_empty());
        assert!(String::from_utf8_lossy(&default.stderr).contains("Bytes resource limit exceeded"));
        let result = run(&[
            "query",
            "run",
            sql,
            "--database",
            db.path(),
            "--native-bytes",
            "67108864",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        assert_eq!(result.stdout, b"Id\n6000\n5999\n5998\n");
    }
}

#[test]
fn native_byte_range_and_command_scope_are_validated_before_io() {
    for bytes in [
        "0",
        "268435457",
        "18446744073709551616",
        "-1",
        "64MiB",
        "1.5",
    ] {
        let output = run(&[
            "query",
            "run",
            "SELECT 1",
            "--database",
            "absent.accdb",
            "--native-bytes",
            bytes,
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("error:"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("database I/O failed"));
    }
    for args in [
        vec!["query", "run", "SELECT 1", "--native-bytes", "67108864"],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--sqlite",
            "absent.sqlite",
            "--native-bytes",
            "67108864",
        ],
        vec!["query", "parse", "SELECT 1", "--native-bytes", "67108864"],
        vec!["query", "eval", "SELECT 1", "--native-bytes", "67108864"],
        vec![
            "query",
            "saved",
            "Saved",
            "--database",
            "absent.accdb",
            "--native-bytes",
            "67108864",
        ],
        vec![
            "query",
            "update",
            "UPDATE Items SET Score=1",
            "--database",
            "absent.mdb",
            "--output",
            "unused.mdb",
            "--native-bytes",
            "67108864",
        ],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("error:"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("database I/O failed"));
    }
}

#[test]
fn minimum_allowance_is_applied_and_maximum_is_accepted_without_changing_rows() {
    let db = Database::new(fixture::database());
    let tiny = run(&[
        "query",
        "run",
        "SELECT Id FROM Items",
        "--database",
        db.path(),
        "--native-bytes",
        "1",
    ]);
    assert_eq!(tiny.status.code(), Some(2));
    assert!(tiny.stdout.is_empty());
    assert!(String::from_utf8_lossy(&tiny.stderr).contains("Bytes resource limit exceeded"));
    let maximum = run(&[
        "query",
        "run",
        "SELECT Id FROM Items ORDER BY Id",
        "--database",
        db.path(),
        "--native-bytes",
        "268435456",
    ]);
    assert!(
        maximum.status.success(),
        "{}",
        String::from_utf8_lossy(&maximum.stderr)
    );
    assert_eq!(maximum.stdout, b"Id\n1\n2\n3\n");
}
