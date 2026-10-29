#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[path = "support/mod.rs"]
mod cli_support;
#[path = "../../../crates/engines/tablecraft-ace/src/preservation/table_extra/tests/fixture.rs"]
mod extra_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/object_payloads/fixture.rs"]
mod payload_fixture;

use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use tablecraft_testkit::access as support;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Database {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl Database {
    fn new(bytes: Vec<u8>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-table-extra-{}-{}.accdb",
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

impl Drop for Database {
    fn drop(&mut self) {
        assert_eq!(std::fs::read(&self.path).unwrap(), self.bytes);
        std::fs::remove_file(&self.path).unwrap();
    }
}

fn local_table(payload: Vec<u8>, duplicate_name: bool) -> Vec<u8> {
    let mut bytes = payload_fixture::database(6, &[None, None, Some(payload)]);
    let page = &bytes[4 * 4096..5 * 4096];
    let start = usize::from(u16::from_le_bytes([page[18], page[19]])) & 0x0fff;
    let end = usize::from(u16::from_le_bytes([page[16], page[17]])) & 0x0fff;
    let mut row = page[start..end].to_vec();
    support::put16(&mut row, 10, 1);
    let mut records = vec![
        (
            0,
            support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        ),
        (
            0,
            support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        ),
        (0, row.clone()),
    ];
    if duplicate_name {
        support::put32(&mut row, 2, 11);
        records.push((0, row));
    }
    support::put32(&mut bytes, 2 * 4096 + 16, records.len() as u32);
    bytes[4 * 4096..5 * 4096].copy_from_slice(&support::data_page(2, &records));
    bytes
}

#[test]
fn extra_inspection_is_explicit_buffered_and_does_not_publish_text() {
    let bytes = local_table(payload_fixture::inline(&extra_fixture::golden()), false);
    let database = Database::new(bytes);
    let output = cli_support::run(&["inspect", database.path(), "--table", "Items", "--extra"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Native ID\t10\n"));
    assert!(text.contains("Catalog page\t4\n"));
    assert!(text.contains("Catalog slot\t2\n"));
    assert!(text.contains("LvExtra field\tPresent\n"));
    assert!(text.contains("Structural coverage\tObserved wide-MR2 framing\n"));
    assert!(text.contains("Data-macro absence\tNot certified\n"));
    assert!(text.contains("\tValidateUpdate\t0x01\t12\t24\tTRUE\n"));
    assert!(!text.contains("BeforeChange"));
    assert!(!text.contains("inert"));

    for arguments in [
        vec!["inspect", database.path(), "--table", "Missing", "--extra"],
        vec![
            "inspect",
            database.path(),
            "--table",
            "Items",
            "--extra",
            "--native-bytes",
            "1",
        ],
        vec![
            "inspect",
            database.path(),
            "--table",
            "Items",
            "--extra",
            "--native-steps",
            "1",
        ],
    ] {
        let output = cli_support::run(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn extra_mode_rejects_every_other_mode_before_opening_a_database() {
    let missing = "tablecraft-extra-missing-database.accdb";
    let without_table = cli_support::run(&["inspect", missing, "--extra"]);
    assert_eq!(without_table.status.code(), Some(2));
    assert!(without_table.stdout.is_empty());
    assert!(String::from_utf8_lossy(&without_table.stderr).contains("--table"));
    for other in [
        vec!["--catalog"],
        vec!["--query", "Query"],
        vec!["--relationships"],
        vec!["--indexes"],
        vec!["--properties"],
        vec!["--features"],
        vec!["--complex"],
        vec!["--stored-calculation", "Field"],
        vec!["--complex-links", "Field"],
        vec!["--attachments", "Field"],
        vec!["--version-history", "Field"],
        vec!["--check"],
        vec!["--constraints"],
    ] {
        let mut args = vec!["inspect", missing, "--table", "Items", "--extra"];
        args.extend(other);
        let output = cli_support::run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used"));
    }
}

#[test]
fn ambiguous_names_refuse_and_external_extra_bytes_remain_opaque() {
    let database = Database::new(local_table(payload_fixture::inline(&[]), true));
    let output = cli_support::run(&["inspect", database.path(), "--table", "Items", "--extra"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ambiguous"));

    let raw = vec![0x61; 65];
    let mut bytes = local_table(payload_fixture::external(&raw), false);
    bytes[13 * 4096..14 * 4096].copy_from_slice(&payload_fixture::lval(&raw));
    let database = Database::new(bytes);
    let output = cli_support::run(&["inspect", database.path(), "--table", "Items", "--extra"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Payload bytes\t65\n"));
    assert!(text.contains("Structural coverage\tOpaque content retained\n"));
    assert!(!text.contains("aaaaaaaa"));
}

#[test]
fn narrow_properties_keep_their_existing_explicit_mode() {
    let database = Database::new(base::with_rule("Description", "existing narrow text"));
    let output = cli_support::run(&[
        "inspect",
        database.path(),
        "--table",
        "Items",
        "--properties",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("existing narrow text")
    );
}
