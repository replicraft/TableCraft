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

#[test]
fn native_row_and_work_controls_are_scoped_and_checked_before_io() {
    for (name, values) in [
        ("--native-rows", vec!["0", "1000001", "-1", "many"]),
        ("--native-steps", vec!["0", "100000001", "-1", "many"]),
    ] {
        for value in values {
            let result = run(&[
                "query",
                "run",
                "SELECT 1",
                "--database",
                "missing-row-budget.accdb",
                name,
                value,
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(name)
                    || String::from_utf8_lossy(&result.stderr).contains("unexpected argument")
            );
        }
        for prefix in [
            vec!["query", "run", "SELECT 1"],
            vec!["query", "run", "SELECT 1", "--sqlite", "missing.sqlite"],
        ] {
            let mut args = prefix;
            args.extend([name, "100000"]);
            let result = run(&args);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains("--database")
                    || String::from_utf8_lossy(&result.stderr).contains("--sqlite")
            );
        }
    }
}

#[test]
fn explicit_query_native_controls_keep_buffering_and_do_not_change_default_profiles() {
    let db = Database::new(fixture::database());
    let expected = run(&[
        "query",
        "run",
        "SELECT Id FROM Items ORDER BY Id",
        "--database",
        db.path(),
    ]);
    assert!(expected.status.success());
    let actual = run(&[
        "query",
        "run",
        "SELECT Id FROM Items ORDER BY Id",
        "--database",
        db.path(),
        "--native-rows",
        "100000",
        "--native-steps",
        "10000000",
    ]);
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(actual.stdout, expected.stdout);
    for name in ["--native-rows", "--native-steps"] {
        let result = run(&[
            "query",
            "run",
            "SELECT Id FROM Items",
            "--database",
            db.path(),
            name,
            "1",
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn execution_work_is_explicitly_bounded_and_separate_from_native_work() {
    for value in ["0", "100000001", "-1", "many"] {
        let result = run(&["query", "run", "SELECT 1", "--execution-steps", value]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    let result = run(&["query", "run", "SELECT 1", "--execution-steps", "10000000"]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"Expr1\n1\n");
    let result = run(&["query", "run", "SELECT 1", "--execution-steps", "1"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn query_allocation_and_row_pools_have_explicit_ranges_and_unchanged_output() {
    for (flag, maximum) in [
        ("--execution-bytes", "268435456"),
        ("--execution-rows", "1000000"),
    ] {
        for value in ["0", "-1", "many", "18446744073709551616"] {
            let result = run(&[
                "query",
                "run",
                "SELECT 1",
                "--database",
                "missing.accdb",
                flag,
                value,
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(flag)
                    || String::from_utf8_lossy(&result.stderr).contains("unexpected argument")
            );
            assert!(!String::from_utf8_lossy(&result.stderr).contains("database I/O failed"));
        }
        let result = run(&["query", "run", "SELECT 1 AS A", flag, maximum]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"A\n1\n");

        let result = run(&["query", "run", "SELECT 1 UNION ALL SELECT 2", flag, "1"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("resource limit exceeded"));
    }
    for (flag, value) in [
        ("--execution-bytes", "268435457"),
        ("--execution-rows", "1000001"),
    ] {
        let result = run(&["query", "run", "SELECT 1", flag, value]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn native_value_allowance_requires_database_and_validates_its_own_range() {
    for value in ["0", "33554433", "-1", "many"] {
        let result = run(&[
            "query",
            "run",
            "SELECT 1",
            "--database",
            "missing.accdb",
            "--native-value-bytes",
            value,
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("database I/O failed"));
    }
    let result = run(&[
        "query",
        "run",
        "SELECT 1",
        "--native-value-bytes",
        "4194304",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("--database"));
}
