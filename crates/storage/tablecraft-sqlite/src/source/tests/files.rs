use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-sqlite-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("database.sqlite")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn wal_read_snapshot_survives_another_connections_commit() {
    let directory = Directory::new();
    let writer = Connection::open(directory.file()).unwrap();
    writer
        .execute_batch(
            "PRAGMA journal_mode=WAL; CREATE TABLE Items(ID INTEGER); INSERT INTO Items VALUES(1)",
        )
        .unwrap();
    let reader =
        SqliteSource::open(directory.file(), budget(), Arc::new(AtomicBool::new(false))).unwrap();
    // Do not scan user rows until after the writer commits. Inventory must have
    // already pinned the snapshot rather than deferring it until the first scan.
    writer.execute("INSERT INTO Items VALUES(2)", []).unwrap();
    assert_eq!(rows(&reader, "Items").unwrap(), vec![vec![Value::Int64(1)]]);
    assert_eq!(rows(&reader, "Items").unwrap(), vec![vec![Value::Int64(1)]]);
    reader.close().unwrap();
    let reader =
        SqliteSource::open(directory.file(), budget(), Arc::new(AtomicBool::new(false))).unwrap();
    assert_eq!(rows(&reader, "Items").unwrap().len(), 2);
    reader.close().unwrap();
    drop(writer);
}

#[test]
fn nonexistent_malformed_and_busy_files_fail_without_creation_or_retry() {
    let directory = Directory::new();
    let open = || SqliteSource::open(directory.file(), budget(), Arc::new(AtomicBool::new(false)));
    assert!(matches!(open(), Err(SqliteError::Database)));
    assert!(!directory.file().exists());
    fs::write(directory.file(), b"not a SQLite file").unwrap();
    assert!(matches!(open(), Err(SqliteError::Database)));
    fs::remove_file(directory.file()).unwrap();

    let writer = Connection::open(directory.file()).unwrap();
    writer
        .execute_batch("CREATE TABLE Items(ID INTEGER); BEGIN EXCLUSIVE")
        .unwrap();
    assert!(matches!(open(), Err(SqliteError::Busy)));
    writer.execute_batch("ROLLBACK").unwrap();
    open().unwrap().close().unwrap();
    drop(writer);
}
