#[path = "../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod fixture;
mod support;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use support::run;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Input {
    path: PathBuf,
    bytes: Vec<u8>,
}
impl Input {
    fn new(code: u8) -> Self {
        let original = fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/examples/Access_Example_VBA.accdb"),
        )
        .unwrap();
        let bytes = fixture::encrypted_image(&original, code, "public 😀 password");
        let path = std::env::temp_dir().join(format!(
            "tablecraft-password-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, &bytes).unwrap();
        Self { path, bytes }
    }
    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
    fn unchanged(&self) {
        assert_eq!(fs::read(&self.path).unwrap(), self.bytes);
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[test]
fn global_password_supports_inventory_rows_queries_and_detailed_inspection() {
    for code in [2, 3, 5, 6] {
        let input = Input::new(code);
        let tables = run(&[
            "-p",
            "public 😀 password",
            "tables",
            input.path(),
            "--count",
        ]);
        assert!(
            tables.status.success(),
            "{}",
            String::from_utf8_lossy(&tables.stderr)
        );
        assert!(String::from_utf8_lossy(&tables.stdout).contains("ROULETTE_TB\tUser\t38"));
        let rows = run(&[
            "rows",
            input.path(),
            "ROULETTE_TB",
            "--limit",
            "3",
            "--password",
            "public 😀 password",
        ]);
        assert!(
            rows.status.success(),
            "{}",
            String::from_utf8_lossy(&rows.stderr)
        );
        assert_eq!(String::from_utf8(rows.stdout).unwrap().lines().count(), 4);
        let query = run(&[
            "query",
            "run",
            "SELECT Count(*) AS Total FROM ROULETTE_TB",
            "--database",
            input.path(),
            "-p",
            "public 😀 password",
        ]);
        assert!(
            query.status.success(),
            "{}",
            String::from_utf8_lossy(&query.stderr)
        );
        assert_eq!(query.stdout, b"Total\n38\n");
        let check = run(&[
            "inspect",
            input.path(),
            "--table",
            "ROULETTE_TB",
            "--check",
            "-p",
            "public 😀 password",
        ]);
        assert!(
            check.status.success(),
            "{}",
            String::from_utf8_lossy(&check.stderr)
        );
        assert!(String::from_utf8_lossy(&check.stdout).contains("Rows checked\t38"));
        input.unchanged();
    }
}

#[test]
fn raw_inspection_needs_no_password_and_failed_credentials_never_publish_data_or_secrets() {
    let input = Input::new(5);
    for command in [
        vec!["inspect", input.path()],
        vec!["encryption", "inspect", input.path()],
    ] {
        let result = run(&command);
        assert!(result.status.success());
    }
    let absent = run(&["tables", input.path()]);
    assert_eq!(absent.status.code(), Some(2));
    assert!(absent.stdout.is_empty());
    assert!(String::from_utf8_lossy(&absent.stderr).contains("database password is required"));
    let wrong = run(&[
        "-vvv",
        "--password",
        "private-sentinel-password",
        "rows",
        input.path(),
        "ROULETTE_TB",
    ]);
    assert_eq!(wrong.status.code(), Some(2));
    assert!(wrong.stdout.is_empty());
    let error = String::from_utf8_lossy(&wrong.stderr);
    assert!(error.contains("incorrect database password"));
    assert!(!error.contains("private-sentinel"));
    assert!(!error.contains(input.path()));
    let limited = run(&[
        "query",
        "run",
        "SELECT * FROM ROULETTE_TB",
        "--database",
        input.path(),
        "--native-bytes",
        "1",
        "-p",
        "public 😀 password",
    ]);
    assert_eq!(limited.status.code(), Some(2));
    assert!(limited.stdout.is_empty());
    input.unchanged();
}
