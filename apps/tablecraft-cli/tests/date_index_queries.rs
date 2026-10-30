use tablecraft_testkit::access::mutation_fixture as base;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/composite_indexes/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

const PAGE_BYTES: usize = 4096;
const INDEX_PAGE: usize = 14;
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Database {
    path: PathBuf,
    original: Vec<u8>,
}

impl Database {
    fn new(bytes: Vec<u8>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-date-index-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&bytes).unwrap();
        Self {
            path,
            original: bytes,
        }
    }

    fn query(&self, sql: &str, options: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["--format", "tsv", "query", "run", "--database"])
            .arg(&self.path)
            .arg(sql)
            .args(options)
            .output()
            .unwrap()
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        assert_eq!(fs::read(&self.path).unwrap(), self.original);
        fs::remove_file(&self.path).unwrap();
    }
}

fn bytes(profile: u8, descending: bool) -> Vec<u8> {
    let values = [
        None,
        Some((-1.25_f64).to_bits()),
        Some((-1.75_f64).to_bits()),
        Some((-0.25_f64).to_bits()),
        Some(0.25_f64.to_bits()),
        Some(0x8000000000000000),
        Some(0),
        Some(1),
        Some(0x8000000000000001),
        Some(0.5_f64.to_bits()),
        Some(1.25_f64.to_bits()),
    ];
    let rows = values
        .iter()
        .enumerate()
        .map(|(index, value)| vec![Some(index as i64 + 1), value.map(|bits| bits as i64)])
        .collect::<Vec<_>>();
    let mut bytes = fixture::database(profile, &[4, 8], &[(1, descending)], &rows, 0);
    if profile == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes
}

fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    output.stdout
}

#[test]
fn automatic_date_queries_preserve_temporal_results_and_sequential_order() {
    for profile in [1, 2, 5, 6] {
        for descending in [false, true] {
            let database = Database::new(bytes(profile, descending));
            for (filter, expected) in [
                ("Score=#12/30/1899 06:00#", &b"Id\n4\n5\n"[..]),
                ("Score=#12/30/1899#", &b"Id\n6\n7\n"[..]),
                (
                    "Score>=#12/29/1899 06:00# AND Score<#12/29/1899 18:00#",
                    &b"Id\n2\n"[..],
                ),
                ("Score IS NULL", &b"Id\n1\n"[..]),
            ] {
                let sql = format!("SELECT Id FROM Items WHERE {filter}");
                assert_eq!(success(database.query(&sql, &[])), expected);
                assert_eq!(success(database.query(&sql, &["--auto-index"])), expected);
            }
            let expected = if descending {
                &b"Id\n5\n4\n"[..]
            } else {
                &b"Id\n4\n5\n"[..]
            };
            assert_eq!(
                success(database.query(
                    "SELECT Id FROM Items WHERE Score=#12/30/1899 06:00#",
                    &["--index", "Items", "Compound"],
                )),
                expected,
            );
        }
    }
}

#[test]
fn iso_typed_date_parameters_use_the_same_safe_index_path() {
    let database = Database::new(bytes(6, true));
    let sql = "PARAMETERS P DATETIME; SELECT Id FROM Items WHERE P=Score";
    for (parameter, expected) in [
        ("P=date:1899-12-30 06:00:00", &b"Id\n4\n5\n"[..]),
        ("P=date:1899-12-30", &b"Id\n6\n7\n"[..]),
        ("P=date:1899-12-29 18:00:00", &b"Id\n3\n"[..]),
    ] {
        assert_eq!(
            success(database.query(sql, &["--auto-index", "--param", parameter])),
            expected,
        );
    }
}

#[test]
fn selected_failures_and_unsafe_filter_errors_remain_buffered() {
    let mut bytes = bytes(6, false);
    bytes[INDEX_PAGE * PAGE_BYTES] = 0;
    let database = Database::new(bytes);
    let sql = "SELECT Id FROM Items WHERE Score=#12/31/1899 06:00#";
    assert_eq!(success(database.query(sql, &[])), b"Id\n11\n");
    let failed = database.query(sql, &["--auto-index"]);
    assert_eq!(failed.status.code(), Some(2));
    assert!(failed.stdout.is_empty());

    let unsafe_filter = database.query(
        "SELECT Id FROM Items WHERE Score=#12/31/1899 06:00# AND 1/0=0",
        &["--auto-index"],
    );
    assert_eq!(unsafe_filter.status.code(), Some(2));
    assert!(unsafe_filter.stdout.is_empty());
    assert!(String::from_utf8_lossy(&unsafe_filter.stderr).contains("division by zero"));
}
