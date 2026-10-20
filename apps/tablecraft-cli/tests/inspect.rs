#[path = "inspect/checks.rs"]
mod checks;
#[path = "inspect/encryption.rs"]
mod encryption;
mod support;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use support::run;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-inspect-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn image() -> Vec<u8> {
    let mut bytes = vec![0; 3 * 4096];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 2;
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    bytes
}
#[test]
fn header_inspection_is_read_only_and_reports_only_qualified_metadata() {
    let bytes = image();
    let fixture = Fixture::new(&bytes);
    let output = run(&["inspect", fixture.path()]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        b"Format\tAce2007\nSubversion\t0\nPage bytes\t4096\nPages\t3\nEncoded pages\tfalse\n"
    );
    assert_eq!(std::fs::read(&fixture.0).unwrap(), bytes);
    let mut encoded = image();
    encoded[62] ^= 1;
    let fixture = Fixture::new(&encoded);
    let output = run(&["inspect", fixture.path()]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Encoded pages\ttrue"));
}
#[test]
fn bad_inputs_and_open_failures_have_no_success_output_or_path_payloads() {
    for bytes in [vec![0; 10], vec![0; 3 * 4096]] {
        let fixture = Fixture::new(&bytes);
        let output = run(&["inspect", fixture.path()]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains(fixture.path()));
    }
    let output = run(&["inspect", "private-missing-db-secret.accdb"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret"));
}

#[test]
fn catalog_inventory_reads_a_disposable_example_copy_and_preserves_native_metadata() {
    let original = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let fixture = Fixture::new(&original);
    let output = run(&["inspect", fixture.path(), "--catalog", "--show-system"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 5 + 1 + 146);
    assert_eq!(
        lines[5],
        "Object ID\tKind\tNative ID\tParent ID\tNative type\tFlags\tName"
    );
    assert!(
        lines[6..]
            .iter()
            .any(|line| line.contains("\tMacro\t") && line.ends_with("\tAutoKeys"))
    );
    assert_eq!(
        lines[6..]
            .iter()
            .filter(|line| line.contains("\tUnknown\t"))
            .count(),
        5
    );
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[test]
fn catalog_failures_never_publish_a_partial_header() {
    let mut newer = image();
    newer[20] = 4;
    let mut encoded = image();
    encoded[62] ^= 1;
    for bytes in [image(), newer, encoded] {
        let fixture = Fixture::new(&bytes);
        let output = run(&["inspect", fixture.path(), "--catalog", "--show-system"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains(fixture.path()));
    }
    let help = run(&["inspect", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--catalog"));
}

#[test]
fn table_schema_inspection_reports_supported_and_unsupported_fields_without_rows() {
    let bytes = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let fixture = Fixture::new(&bytes);
    let output = run(&["inspect", fixture.path(), "--table", "ROULETTE_TB"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Table\tROULETTE_TB\nDeclared rows\t38\nScalar columns\tSupported carriers; rows not inspected\nUnsupported columns\t0\nColumn\tNative type\tStorage bytes\tFlags\tExtended flags\tScalar carrier\nORDR_NBR\t4\t4\t0x03\t0x00\tInt32\nNBR_VALUE\t10\t4\t0x02\t0x00\tText\nCOLOR_ID\t4\t4\t0x03\t0x00\tInt32\n"
    );
    let output = run(&["inspect", fixture.path(), "--table", "MSysResources"]);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Unsupported")
    );
    for args in [
        vec!["inspect", fixture.path(), "--table", "missing-secret-name"],
        vec![
            "inspect",
            fixture.path(),
            "--catalog",
            "--table",
            "ROULETTE_TB",
        ],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    assert_eq!(std::fs::read(&fixture.0).unwrap(), bytes);
}
