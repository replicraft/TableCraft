mod support;

use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use support::run;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    path: PathBuf,
    bytes: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-cli-sqlite-{}-{}.sqlite",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(!path.exists());
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE Items(ID INTEGER, Label TEXT);
            INSERT INTO Items VALUES(1,'one'),(2,'two'),(3,'three');
            CREATE TABLE Mixed(ID INTEGER); INSERT INTO Mixed VALUES(1),('private-payload');",
            )
            .unwrap();
        connection.close().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        Self { path, bytes }
    }
    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
    fn query(&self, sql: &str) -> std::process::Output {
        run(&["query", "run", "--sqlite", self.path(), sql])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(std::fs::read(&self.path).unwrap(), self.bytes);
        std::fs::remove_file(&self.path).unwrap();
    }
}

#[test]
fn sqlite_queries_use_access_top_concat_parameters_and_join_semantics() {
    let fixture = Fixture::new();
    let output = fixture
        .query("SELECT TOP 2 ID, Label & '!' AS Caption FROM Items WHERE ID >= 2 ORDER BY ID DESC");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"ID\tCaption\n3\t\"\"\"three!\"\"\"\n2\t\"\"\"two!\"\"\"\n"
    );
    let output = run(&[
        "query",
        "run",
        "--sqlite",
        fixture.path(),
        "--param",
        "Floor=bigint:1",
        "PARAMETERS Floor BIGINT; SELECT Count(*) AS N FROM Items WHERE ID > Floor",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"N\n2\n");
    let output = fixture
        .query("SELECT a.ID FROM Items AS a INNER JOIN Items AS b ON a.ID=b.ID ORDER BY a.ID");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"ID\n1\n2\n3\n");
}

#[test]
fn sqlite_errors_are_buffered_redacted_and_never_execute_writes() {
    let fixture = Fixture::new();
    for sql in [
        "SELECT * FROM Mixed",
        "DELETE FROM Items",
        "SELECT * FROM [private-table]",
    ] {
        let output = fixture.query(sql);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        for private in [fixture.path(), "private-table", "private-payload"] {
            assert!(!error.contains(private));
        }
    }
    for extra in [
        vec!["--database", "unused.accdb"],
        vec!["--index", "Items", "Key"],
    ] {
        let mut arguments = vec![
            "query",
            "run",
            "--sqlite",
            fixture.path(),
            "SELECT * FROM Items",
        ];
        arguments.extend(extra);
        assert_eq!(run(&arguments).status.code(), Some(2));
    }
}
