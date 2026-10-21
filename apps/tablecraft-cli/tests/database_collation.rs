mod support;
use std::{
    io::Write,
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
    fn new(order: Option<[u8; 4]>) -> Self {
        let mut bytes = include_bytes!("../../../docs/examples/Access_Example_VBA.accdb").to_vec();
        if let Some(order) = order {
            for (i, mask) in [1, 27, 152, 253].into_iter().enumerate() {
                bytes[110 + i] = order[i] ^ mask;
            }
        }
        let path = std::env::temp_dir().join(format!(
            "tablecraft-database-collation-{}-{}.accdb",
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
    fn query(&self, sql: &str, extra: &[&str]) -> std::process::Output {
        let mut args = vec![
            "query",
            "run",
            sql,
            "--database",
            self.path(),
            "--collation",
            "database",
        ];
        args.extend(extra);
        run(&args)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(std::fs::read(&self.path).unwrap(), self.bytes);
        std::fs::remove_file(&self.path).unwrap();
    }
}
#[test]
fn database_policy_drives_queries_and_selected_indexes_without_becoming_default() {
    let fixture = Fixture::new(None);
    for extra in [vec![], vec!["--index", "ROULETTE_TB", "ORDR_NBR"]] {
        let result=fixture.query("SELECT TOP 1 ORDR_NBR FROM ROULETTE_TB WHERE NBR_VALUE LIKE '2*' AND 'a'='A' ORDER BY ORDR_NBR",&[extra,vec!["--like-mode","ansi89"]].concat());
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"ORDR_NBR\n1\n");
    }
    let result = fixture.query(
        "SELECT 'a'='A ' AS Eq, 'ss' LIKE '[ß]' AS MatchValue, InStr(1,'Straße','ss',2) AS Found",
        &["--like-mode", "ansi89"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"Eq\tMatchValue\tFound\nTRUE\tTRUE\t5\n");
    let absent = run(&[
        "query",
        "run",
        "SELECT 'a'='A' AS R",
        "--database",
        fixture.path(),
    ]);
    assert!(!absent.status.success());
    assert!(absent.stdout.is_empty());
}
#[test]
fn unknown_or_mixed_orders_fail_with_buffered_payload_free_errors() {
    for descriptor in [
        [0, 0, 0, 0],
        [9, 4, 1, 1],
        [9, 4, 0, 2],
        [31, 4, 0, 1],
        [9, 4, 0, 1],
    ] {
        let fixture = Fixture::new(Some(descriptor));
        let result = fixture.query("SELECT ORDR_NBR FROM ROULETTE_TB WHERE FALSE", &[]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        let error = String::from_utf8(result.stderr).unwrap();
        for payload in [fixture.path(), "ROULETTE_TB", "ORDR_NBR"] {
            assert!(!error.contains(payload));
        }
    }
}
#[test]
fn database_policy_requires_native_input_and_rejects_unresolved_collections_and_writes() {
    for args in [
        vec!["query", "run", "SELECT 1", "--collation", "database"],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--collation",
            "database",
            "--sqlite",
            "never-opened.sqlite",
        ],
        vec![
            "query",
            "update",
            "UPDATE T SET ID=1",
            "--database",
            "never-opened.mdb",
            "--output",
            "never-created.mdb",
            "--collation",
            "database",
        ],
    ] {
        let result = run(&args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
    let fixture = Fixture::new(None);
    for flag in ["--multivalues", "--collections", "--decoded-attachments"] {
        let result = fixture.query("SELECT 1", &[flag]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
    let result = run(&["query", "run", "--help"]);
    assert!(result.status.success());
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("database")
    );
}
