#[path = "support/mod.rs"]
mod cli_support;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/narrow_indexes/fixture.rs"]
mod narrow_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/ace16/scalar_fixture.rs"]
mod scalar_fixture;
use tablecraft_testkit::access as support;

use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Database(PathBuf, Vec<u8>);
impl Database {
    fn new(kind: u8, width: usize, values: &[Option<Vec<u8>>]) -> Self {
        let bytes = scalar_fixture::fixed_scalar(6, kind, width, values);
        Self::from_bytes(bytes)
    }
    fn from_bytes(bytes: Vec<u8>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-modern-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, &bytes).unwrap();
        Self(path, bytes)
    }
    fn query(&self, sql: &str, extra: &[&str]) -> std::process::Output {
        let mut args = vec!["query", "run", sql, "--database", self.0.to_str().unwrap()];
        args.extend(extra);
        cli_support::run(&args)
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        assert_eq!(std::fs::read(&self.0).unwrap(), self.1);
        std::fs::remove_file(&self.0).unwrap();
    }
}

#[test]
fn selected_narrow_indexes_keep_unsigned_and_signed_carriers_through_sql_filters() {
    for (small, values, all, filtered) in [
        (
            false,
            vec![Some(0), Some(127), Some(128), Some(255), None],
            "Score\nNULL\n0\n127\n128\n255\n",
            "Score\n128\n255\n",
        ),
        (
            true,
            vec![Some(-32768), Some(-1), Some(0), Some(32767), None],
            "Score\nNULL\n-32768\n-1\n0\n32767\n",
            "Score\n32767\n",
        ),
    ] {
        for descending in [false, true] {
            let db =
                Database::from_bytes(narrow_fixture::database(6, small, descending, &values, 0));
            for (sql, expected) in [
                ("SELECT Score FROM Items ORDER BY Score", all),
                (
                    "SELECT Score FROM Items WHERE Score>=128 ORDER BY Score",
                    filtered,
                ),
            ] {
                let result = db.query(sql, &["--index", "Items", "BigKey"]);
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert_eq!(result.stdout, expected.as_bytes());
            }
        }
    }
}

#[test]
fn selected_int64_indexes_retain_exact_filters_nulls_and_failure_atomicity() {
    let numbers = [
        Some(i64::MIN),
        Some(9_007_199_254_740_993),
        None,
        Some(i64::MAX),
    ];
    for descending in [false, true] {
        let db = Database::from_bytes(int64_index_fixture::database(6, descending, &numbers));
        for (sql, expected) in [
            (
                "SELECT Score FROM Items WHERE Score IS NULL",
                "Score\nNULL\n",
            ),
            (
                "SELECT Score FROM Items WHERE Score > 9007199254740992 ORDER BY Score",
                "Score\n9007199254740993\n9223372036854775807\n",
            ),
            (
                "SELECT Score FROM Items WHERE Score > 0 ORDER BY Score",
                "Score\n9007199254740993\n9223372036854775807\n",
            ),
            (
                "SELECT Score FROM Items WHERE Score=-9223372036854775808",
                "Score\n-9223372036854775808\n",
            ),
            (
                "SELECT Score FROM Items WHERE Score=9007199254740993",
                "Score\n9007199254740993\n",
            ),
            (
                "SELECT Score FROM Items WHERE Score=9007199254740992.0",
                "Score\n9007199254740993\n",
            ),
            (
                "SELECT Score FROM Items WHERE Score=9007199254740992",
                "Score\n",
            ),
        ] {
            let result = db.query(sql, &["--index", "Items", "BigKey"]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, expected.as_bytes());
        }
        let parameter = db.query(
            "PARAMETERS p BIGINT; SELECT Score FROM Items WHERE Score=p",
            &[
                "--index",
                "Items",
                "BigKey",
                "--param",
                "p=bigint:9007199254740993",
            ],
        );
        assert!(
            parameter.status.success(),
            "{}",
            String::from_utf8_lossy(&parameter.stderr)
        );
        assert_eq!(parameter.stdout, b"Score\n9007199254740993\n");
        let result = db.query(
            "SELECT Score FROM Items WHERE Score>0 AND 1/0=0",
            &["--index", "Items", "BigKey"],
        );
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        let result = db.query(
            "SELECT Score FROM Items WHERE Score IS NULL",
            &["--index", "Items", "missing"],
        );
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    let mut bytes = int64_index_fixture::database(6, false, &numbers);
    bytes[14 * 4096] = 0;
    let db = Database::from_bytes(bytes);
    let result = db.query(
        "SELECT Score FROM Items WHERE Score IS NULL",
        &["--index", "Items", "BigKey"],
    );
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn modern_large_numbers_filter_and_order_without_double_rounding() {
    let values = [
        Some(i64::MAX),
        None,
        Some(9_007_199_254_740_993),
        Some(i64::MIN),
    ];
    let bytes = values.map(|value| value.map(|n| n.to_le_bytes().to_vec()));
    let db = Database::new(0x13, 8, &bytes);
    let result = db.query("SELECT Score FROM Items ORDER BY Score", &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"Score\nNULL\n-9223372036854775808\n9007199254740993\n9223372036854775807\n"
    );
    let result = db.query(
        "PARAMETERS Floor BIGINT; SELECT Score FROM Items WHERE Score > Floor ORDER BY Score",
        &["--param", "Floor=bigint:9007199254740992"],
    );
    assert!(result.status.success());
    assert_eq!(
        result.stdout,
        b"Score\n9007199254740993\n9223372036854775807\n"
    );
}

#[test]
fn modern_dates_order_and_render_exact_fraction_and_reject_malformed_rows() {
    let early = b"0000000000000000000:0000000000000000000:7\0".to_vec();
    let late = format!("{:019}:{:012}{:07}:7\0", 3_652_058, 86_399, 9_999_999).into_bytes();
    let db = Database::new(0x14, 42, &[Some(late), None, Some(early.clone())]);
    let result = db.query("SELECT Score FROM Items ORDER BY Score", &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"Score\nNULL\n#0001-01-01 00:00:00.0000000#\n#9999-12-31 23:59:59.9999999#\n"
    );
    let mut malformed = early.clone();
    malformed[0] = b'x';
    let db = Database::new(0x14, 42, &[Some(early), Some(malformed)]);
    let result = db.query("SELECT * FROM Items", &[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
