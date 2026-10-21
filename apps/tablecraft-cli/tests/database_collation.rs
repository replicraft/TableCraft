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
    fn column_order(order: [u8; 4]) -> Self {
        let mut fixture = Self::new(None);
        let schema = tablecraft_ace::inspect_table_schema(
            &mut std::io::Cursor::new(&fixture.bytes),
            tablecraft_model::CatalogScope::new(1),
            "ROULETTE_TB",
            &mut tablecraft_core::limits::ResourceBudget::new(16 * 1024 * 1024, 10000, 1_000_000),
            &|| false,
        )
        .unwrap();
        let page = schema.definition_page() as usize * 4096;
        let indexes =
            u32::from_le_bytes(fixture.bytes[page + 51..page + 55].try_into().unwrap()) as usize;
        let at = page + 63 + indexes * 12 + 25 + 11;
        fixture.bytes[at..at + 4].copy_from_slice(&order);
        std::fs::write(&fixture.path, &fixture.bytes).unwrap();
        fixture
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
fn unknown_database_orders_fail_with_buffered_payload_free_errors() {
    for descriptor in [[0, 0, 0, 0], [9, 4, 1, 1], [9, 4, 0, 2], [31, 4, 0, 1]] {
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

#[test]
fn differing_known_database_and_field_families_can_be_used_in_one_native_query() {
    let fixture = Fixture::new(Some([9, 4, 0, 1]));
    let result = fixture.query(
        "SELECT TOP 1 ORDR_NBR FROM ROULETTE_TB WHERE NBR_VALUE='2' AND 'a'='A' ORDER BY ORDR_NBR",
        &[],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"ORDR_NBR\n1\n");
}

#[test]
fn unknown_field_orders_allow_projection_but_never_silently_compare() {
    let known = Fixture::new(None);
    for raw in [[0, 0, 0, 0], [9, 4, 1, 1], [9, 4, 0, 2], [31, 4, 0, 1]] {
        let fixture = Fixture::column_order(raw);
        let sql = "SELECT NBR_VALUE FROM ROULETTE_TB WHERE ORDR_NBR=1";
        let actual = fixture.query(sql, &[]);
        let expected = known.query(sql, &[]);
        assert!(
            actual.status.success(),
            "{}",
            String::from_utf8_lossy(&actual.stderr)
        );
        assert_eq!(actual.stdout, expected.stdout);
        let rejected = fixture.query(
            "SELECT NBR_VALUE='2' FROM ROULETTE_TB WHERE ORDR_NBR=1",
            &[],
        );
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&rejected.stderr).contains(fixture.path()));
        let override_result = run(&[
            "query",
            "run",
            "SELECT NBR_VALUE='2' AS R FROM ROULETTE_TB WHERE ORDR_NBR=1",
            "--database",
            fixture.path(),
            "--collation",
            "general-2010",
        ]);
        assert!(override_result.status.success());
        assert_eq!(override_result.stdout, b"R\nTRUE\n");
    }
}
