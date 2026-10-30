use tablecraft_testkit::access as native_support;
// The independent golden builder uses its owner's support module name.
use native_support as support;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/index_fixture.rs"]
mod index_fixture;

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Files(PathBuf);
impl Files {
    fn new(bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-cli-update-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("input.mdb"), bytes).unwrap();
        Self(path)
    }
    fn update(&self, sql: &str, output: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "update", sql, "--database"])
            .arg(self.0.join("input.mdb"))
            .arg("--output")
            .arg(self.0.join(output))
            .output()
            .unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn cli_update_creates_a_reopenable_copy_with_exact_original_preservation() {
    let before = fixture::with_rule("ValidationRule", ">=0 And <=100");
    let files = Files::new(&before);
    let output = files.update(
        "UPDATE Items AS i SET i.Score=i.Score+7 WHERE i.Id=2",
        "updated.mdb",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"Matched rows\t1\nChanged rows\t1\n");
    assert_eq!(fs::read(files.0.join("input.mdb")).unwrap(), before);
    let after = fs::read(files.0.join("updated.mdb")).unwrap();
    assert_eq!(
        before
            .iter()
            .zip(&after)
            .filter(|(a, b)| a != b)
            .map(|(a, b)| (*a, *b))
            .collect::<Vec<_>>(),
        [(20, 27)]
    );
    let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "run",
            "SELECT Score FROM Items WHERE Id=2",
            "--database",
        ])
        .arg(files.0.join("updated.mdb"))
        .output()
        .unwrap();
    assert!(read.status.success());
    assert_eq!(read.stdout, b"Score\n27\n");
}

#[test]
fn unique_index_updates_reopen_and_rejected_layouts_create_no_output() {
    let before = index_fixture::database(false, true, false);
    let files = Files::new(&before);
    let output = files.update("UPDATE Items SET Score=35 WHERE Id=2", "indexed.mdb");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Matched rows\t1\nChanged rows\t1\n");
    assert_eq!(fs::read(files.0.join("input.mdb")).unwrap(), before);
    let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "run",
            "SELECT Score FROM Items ORDER BY Score",
            "--database",
        ])
        .arg(files.0.join("indexed.mdb"))
        .args(["--index", "Items", "ScoreAlias"])
        .output()
        .unwrap();
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    assert_eq!(read.stdout, b"Score\n10\n30\n35\n");
    let rejected = files.update("UPDATE Items SET Score=30 WHERE Id=2", "duplicate.mdb");
    assert_eq!(rejected.status.code(), Some(2));
    assert!(rejected.stdout.is_empty());
    assert!(!files.0.join("duplicate.mdb").exists());
    let nodes = Files::new(&index_fixture::node_database());
    let updated = nodes.update("UPDATE Items SET Score=35 WHERE Id=2", "node.mdb");
    assert!(
        updated.status.success(),
        "{}",
        String::from_utf8_lossy(&updated.stderr)
    );
    assert_eq!(updated.stdout, b"Matched rows\t1\nChanged rows\t1\n");
    let mut unknown = index_fixture::node_database();
    unknown[13 * 4096 + 26] = 0;
    let unknown = Files::new(&unknown);
    let rejected = unknown.update("UPDATE Items SET Score=35 WHERE Id=2", "unknown.mdb");
    assert_eq!(rejected.status.code(), Some(2));
    assert!(rejected.stdout.is_empty());
    assert!(!unknown.0.join("unknown.mdb").exists());
}

#[test]
fn rejected_queries_create_no_output_and_existing_paths_are_never_overwritten() {
    let bytes = fixture::with_rule("ValidationRule", ">=0 And <=100");
    let files = Files::new(&bytes);
    for sql in [
        "UPDATE Items SET Score=101 WHERE Id=2",
        r"UPDATE Items SET Score=10 \ (Id-2)",
        "UPDATE Items SET Score=NULL",
        "UPDATE Items SET Score=Missing",
        "DELETE FROM Items",
    ] {
        let output = files.update(sql, "rejected.mdb");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!files.0.join("rejected.mdb").exists());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(!diagnostic.contains(sql));
        assert!(!diagnostic.contains(files.0.to_str().unwrap()));
    }
    fs::write(files.0.join("exists.mdb"), b"keep").unwrap();
    let lock = tablecraft_io::ExclusiveFile::open(&files.0.join("input.mdb")).unwrap();
    let busy = files.update("UPDATE Items SET Score=1", "busy.mdb");
    assert_eq!(busy.status.code(), Some(2));
    assert!(busy.stdout.is_empty());
    assert!(!files.0.join("busy.mdb").exists());
    lock.close().unwrap();
    assert_eq!(
        files
            .update("UPDATE Items SET Score=1", "exists.mdb")
            .status
            .code(),
        Some(2)
    );
    assert_eq!(fs::read(files.0.join("exists.mdb")).unwrap(), b"keep");
    assert_eq!(
        files
            .update("UPDATE Items SET Score=1", "input.mdb")
            .status
            .code(),
        Some(2)
    );
    assert_eq!(fs::read(files.0.join("input.mdb")).unwrap(), bytes);
}

#[test]
fn ace_and_unavailable_properties_are_rejected_before_output_creation() {
    for (ace, mut bytes) in [
        (true, fixture::database()),
        (false, fixture::with_rule("FutureRule", "0")),
    ] {
        if ace {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
            bytes[20] = 2;
        }
        let files = Files::new(&bytes);
        let output = files.update("UPDATE Items SET Score=1", "rejected.mdb");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!files.0.join("rejected.mdb").exists());
        assert_eq!(fs::read(files.0.join("input.mdb")).unwrap(), bytes);
    }
}
