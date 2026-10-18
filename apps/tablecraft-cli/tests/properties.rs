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
        let bytes = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/examples/Access_Example_VBA.accdb"),
        )
        .unwrap();
        let path = std::env::temp_dir().join(format!(
            "tablecraft-properties-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, &bytes).unwrap();
        Self { path, bytes }
    }
    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(std::fs::read(&self.path).unwrap(), self.bytes);
        std::fs::remove_file(&self.path).unwrap();
    }
}

#[test]
fn property_inspection_reports_declarations_and_keeps_opaque_values_explicit() {
    let fixture = Fixture::new();
    let output = run(&[
        "inspect",
        fixture.path(),
        "--table",
        "ROULETTE_TB",
        "--properties",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Column\tNBR_VALUE\tRequired\t1\t0x01\tTRUE\n"));
    assert!(text.contains("Column\tNBR_VALUE\tAllowZeroLength\t1\t0x01\tFALSE\n"));
    assert!(text.contains("Table\t\tGUID\t9\t0x01\tOPAQUE\n"));
}

#[test]
fn failed_property_inspection_is_buffered_and_options_require_a_table() {
    let fixture = Fixture::new();
    for arguments in [
        vec!["inspect", fixture.path(), "--properties"],
        vec![
            "inspect",
            fixture.path(),
            "--table",
            "ROULETTE_TB",
            "--properties",
            "--indexes",
        ],
        vec![
            "inspect",
            fixture.path(),
            "--table",
            "ROULETTE_TB",
            "--properties",
            "--check",
        ],
        vec![
            "inspect",
            fixture.path(),
            "--table",
            "private-name",
            "--properties",
        ],
    ] {
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let output = run(&[
        "-vv",
        "inspect",
        fixture.path(),
        "--table",
        "ROULETTE_TB",
        "--properties",
    ]);
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    for private in [
        fixture.path(),
        "ROULETTE_TB",
        "NBR_VALUE",
        "Numbers for roulette",
    ] {
        assert!(!diagnostics.contains(private));
    }
}
