#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
#[allow(dead_code)]
mod base;
#[path = "support/mod.rs"]
mod cli_support;
#[path = "../../../crates/engines/tablecraft-ace/tests/saved_queries/fixture.rs"]
mod fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use std::{
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
            "tablecraft-saved-query-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&bytes).unwrap();
        Self { path, bytes }
    }
    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        assert_eq!(std::fs::read(&self.path).unwrap(), self.bytes);
        std::fs::remove_file(&self.path).unwrap();
    }
}

#[test]
fn explicit_saved_query_inspection_retains_inert_fragments_and_quoted_null_distinctions() {
    let db = Database::new(fixture::database());
    let result = cli_support::run(&["inspect", db.path(), "--query", "Saved"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.starts_with(&format!(
        "Query\tSaved\nNative ID\t{}\nFlags\t0x80000030\nFragment rows\t2\n",
        fixture::QUERY_ID
    )));
    assert!(text.contains(
        "200\t\"NeverRunMe()\\t\\n\\u{0000}é🦀\"\t-32768\t-2147483648\t\"NULL\"\t\"\"\t0x09080706\n"
    ));
    assert!(text.ends_with("NULL\tNULL\tNULL\tNULL\tNULL\tNULL\t0x\n"));
    let result = cli_support::run(&["-vv", "inspect", db.path(), "--query", "Saved"]);
    assert!(result.status.success());
    let diagnostics = String::from_utf8(result.stderr).unwrap();
    for private in [db.path(), "Saved", "NeverRunMe"] {
        assert!(!diagnostics.contains(private));
    }
}

#[test]
fn saved_query_mode_conflicts_and_non_query_names_do_not_fall_back_to_other_inspection() {
    let db = Database::new(fixture::database());
    for options in [
        vec!["--catalog"],
        vec!["--relationships"],
        vec!["--table", "Items"],
        vec!["--indexes"],
        vec!["--properties"],
        vec!["--features"],
        vec!["--complex"],
        vec!["--stored-calculation", "Score"],
        vec!["--complex-links", "Score"],
        vec!["--check"],
        vec!["--constraints"],
    ] {
        let mut args = vec!["inspect", db.path(), "--query", "Saved"];
        args.extend(options);
        let result = cli_support::run(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(result.stdout.is_empty());
    }
    for name in ["Items", "missing"] {
        let result = cli_support::run(&["inspect", db.path(), "--query", name]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    let result = cli_support::run(&["inspect", db.path(), "--query", "Empty"]);
    assert!(result.status.success());
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("Fragment rows\t0\n")
    );
}

#[test]
fn late_system_table_corruption_publishes_no_query_metadata() {
    let mut bytes = fixture::database();
    support::put32(
        &mut bytes,
        fixture::DEFINITION_PAGE * fixture::PAGE_BYTES + 16,
        4,
    );
    let db = Database::new(bytes);
    let result = cli_support::run(&["inspect", db.path(), "--query", "Saved"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
