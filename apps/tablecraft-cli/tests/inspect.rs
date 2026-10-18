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
