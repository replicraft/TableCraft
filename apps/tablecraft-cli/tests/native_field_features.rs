#[path = "support/mod.rs"]
mod cli_support;
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

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
