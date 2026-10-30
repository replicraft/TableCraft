#[path = "support/mod.rs"]
mod cli_support;
use fixture as base;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/calculated_storage/scalar_fixture.rs"]
mod calculated_scalar_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;

use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Database(PathBuf, Vec<u8>);
impl Database {
    fn new(key: &str, text: &str) -> Self {
        let mut bytes = fixture::with_rule(key, text);
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        bytes[20] = 6;
        bytes[10 * 4096 + 63 + 25 + 16] = 0xc0;
        Self::from_bytes(bytes)
    }
    fn from_bytes(bytes: Vec<u8>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-field-features-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, &bytes).unwrap();
        Self(path, bytes)
    }
    fn inspect(&self) -> std::process::Output {
        cli_support::run(&[
            "inspect",
            self.0.to_str().unwrap(),
            "--table",
            "Items",
            "--features",
        ])
    }
}

#[test]
fn stored_calculation_inspection_is_explicit_and_buffers_late_failures() {
    let bytes = calculated_fixture::database(&[Some(20), Some(41), None]);
    let db = Database::from_bytes(bytes.clone());
    let args = [
        "inspect",
        db.0.to_str().unwrap(),
        "--table",
        "Items",
        "--stored-calculation",
        "Score",
    ];
    let result = cli_support::run(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "Table\tItems\nColumn\tScore\nValues\tStored; not recalculated\nRow ordinal\tStored Int32 value\n0\t20\n1\t41\n2\tNULL\n"
    );
    let mut broken = bytes;
    let second =
        u16::from_le_bytes(broken[12 * 4096 + 16..12 * 4096 + 18].try_into().unwrap()) as usize;
    broken[12 * 4096 + second + 6] = 1;
    let db = Database::from_bytes(broken);
    let result = cli_support::run(&[
        "inspect",
        db.0.to_str().unwrap(),
        "--table",
        "Items",
        "--stored-calculation",
        "Score",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    for args in [
        vec!["inspect", "missing", "--stored-calculation", "Score"],
        vec![
            "inspect",
            "missing",
            "--table",
            "Items",
            "--features",
            "--stored-calculation",
            "Score",
        ],
    ] {
        let result = cli_support::run(&args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn stored_boolean_and_text_inspection_preserves_null_empty_and_buffers_unknown_payloads() {
    for (kind, capacity, payloads, expected) in [
        (
            1,
            39,
            vec![Some(vec![0xff]), Some(vec![0]), None, Some(vec![])],
            "Stored Boolean value\n0\tTRUE\n1\tFALSE\n2\tNULL\n3\tFALSE\n",
        ),
        (
            10,
            103,
            vec![
                Some(
                    "é🦀\t\n"
                        .encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect(),
                ),
                Some(vec![]),
                None,
            ],
            "Stored Text value\n0\t\"\"\"é🦀\\t\\n\"\"\"\n1\t\"\"\"\"\"\"\n2\tNULL\n",
        ),
    ] {
        let db = Database::from_bytes(calculated_scalar_fixture::database(
            6, kind, capacity, false, &payloads,
        ));
        let result = cli_support::run(&[
            "inspect",
            db.0.to_str().unwrap(),
            "--table",
            "Items",
            "--stored-calculation",
            "Score",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            format!(
                "Table\tItems\nColumn\tScore\nValues\tStored; not recalculated\nRow ordinal\t{expected}"
            )
        );
    }
    let mut bytes =
        calculated_scalar_fixture::database(6, 1, 39, false, &[Some(vec![0xff]), Some(vec![0])]);
    let second =
        u16::from_le_bytes(bytes[12 * 4096 + 16..12 * 4096 + 18].try_into().unwrap()) as usize;
    bytes[12 * 4096 + second + 6 + 20] = 1;
    let db = Database::from_bytes(bytes);
    let result = cli_support::run(&[
        "inspect",
        db.0.to_str().unwrap(),
        "--table",
        "Items",
        "--stored-calculation",
        "Score",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn complex_links_are_explicit_and_orphans_fail_without_partial_output() {
    let bytes = links_fixture::populated();
    let db = Database::from_bytes(bytes.clone());
    let result = cli_support::run(&[
        "inspect",
        db.0.to_str().unwrap(),
        "--table",
        "Items",
        "--complex-links",
        "Score",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "Table\tItems\nColumn\tScore\nChild table\tChildren\nParent ordinal\tComplex key\tRaw child rows\n0\t10\t2\n1\t20\t1\n2\t30\t0\n"
    );
    let mut broken = bytes;
    let second =
        u16::from_le_bytes(broken[22 * 4096 + 16..22 * 4096 + 18].try_into().unwrap()) as usize;
    support::put32(&mut broken, 22 * 4096 + second + 2, 999);
    let db = Database::from_bytes(broken);
    let result = cli_support::run(&[
        "inspect",
        db.0.to_str().unwrap(),
        "--table",
        "Items",
        "--complex-links",
        "Score",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    for args in [
        vec!["inspect", "missing", "--complex-links", "Score"],
        vec![
            "inspect",
            "missing",
            "--table",
            "Items",
            "--features",
            "--complex-links",
            "Score",
        ],
    ] {
        let result = cli_support::run(&args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        assert_eq!(std::fs::read(&self.0).unwrap(), self.1);
        std::fs::remove_file(&self.0).unwrap();
    }
}
#[test]
fn inspect_reports_calculation_without_executing_or_claiming_a_scalar_result() {
    let result = Database::new("Expression", "[Id]*2").inspect();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8(result.stdout).unwrap().contains(
            "Score\tTRUE\tNONE\tFALSE\tFALSE\tFALSE\tUNKNOWN\tUNKNOWN\tUNKNOWN\t[Id]*2\n"
        )
    );
}
#[test]
fn malformed_declaration_fails_without_partial_feature_output() {
    let result = Database::new("AppendOnly", "private-flag-text").inspect();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&result.stderr).contains("private-flag-text"));
    for args in [
        vec!["inspect", "missing", "--features"],
        vec![
            "inspect",
            "missing",
            "--table",
            "Items",
            "--features",
            "--properties",
        ],
    ] {
        let result = cli_support::run(&args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn complex_schema_mode_resolves_references_and_rejects_corrupt_mapping_without_output() {
    let mut db = Database::new("Caption", "complex");
    db.1 = complex_fixture::database();
    std::fs::write(&db.0, &db.1).unwrap();
    let args = [
        "inspect",
        db.0.to_str().unwrap(),
        "--table",
        "Items",
        "--complex",
    ];
    let result = cli_support::run(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("Score\t7\tValueType\tChildren\tOwner\tChildID\t0\n")
    );
    db.1 = complex_fixture::with_mappings(vec![complex_fixture::mapping(7, 99, 20, 17)]);
    std::fs::write(&db.0, &db.1).unwrap();
    let result = cli_support::run(&args);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
