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
            "tablecraft-record-check-{}-{}.accdb",
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
fn explicit_audit_reports_each_field_and_foreign_key_without_changing_normal_reads() {
    let fixture = Fixture::new();
    let output = run(&[
        "inspect",
        fixture.path(),
        "--table",
        "TAG_NME_TB",
        "--constraints",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Rows checked\t933\n"));
    assert!(text.contains("TAG_GRP_ID\tChecked\tNot declared\tNot declared\n"));
    assert!(text.contains("TAG_GRP_TBTAG_NME_TB\tChecked: Int32 key membership\n"));
    let output = run(&[
        "-vv",
        "inspect",
        fixture.path(),
        "--table",
        "TAG_NME_TB",
        "--constraints",
    ]);
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert!(!diagnostics.contains(fixture.path()));
    assert!(!diagnostics.contains("TAG_NME_TB"));
}

#[test]
fn constraint_audits_require_a_table_and_keep_failed_results_buffered() {
    let fixture = Fixture::new();
    for args in [
        vec!["inspect", fixture.path(), "--constraints"],
        vec![
            "inspect",
            fixture.path(),
            "--table",
            "TAG_NME_TB",
            "--constraints",
            "--check",
        ],
        vec![
            "inspect",
            fixture.path(),
            "--table",
            "TAG_NME_TB",
            "--constraints",
            "--relationships",
        ],
        vec![
            "inspect",
            fixture.path(),
            "--table",
            "private-table",
            "--constraints",
        ],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}
