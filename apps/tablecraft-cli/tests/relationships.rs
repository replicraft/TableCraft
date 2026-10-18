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
            "tablecraft-relationships-{}-{}.accdb",
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
fn relationship_cli_reports_declarations_with_no_active_content_or_writes() {
    let fixture = Fixture::new();
    let output = run(&["inspect", fixture.path(), "--relationships"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("TAG_GRP_TBTAG_NME_TB\tTAG_GRP_TB\tTAG_NME_TB\t0x00000000\tTRUE\tFALSE\tRestrict\tRestrict\t0\tTAG_GRP_ID\tTAG_GRP_ID\n"));
    let verbose = run(&["-vv", "inspect", fixture.path(), "--relationships"]);
    let diagnostics = String::from_utf8(verbose.stderr).unwrap();
    assert!(!diagnostics.contains("TAG_GRP_TB"));
    assert!(!diagnostics.contains(fixture.path()));
}

#[test]
fn relationships_conflict_with_other_inspection_modes_before_reading() {
    let fixture = Fixture::new();
    for other in [
        vec!["--catalog"],
        vec!["--table", "ROULETTE_TB"],
        vec!["--table", "ROULETTE_TB", "--properties"],
    ] {
        let mut arguments = vec!["inspect", fixture.path(), "--relationships"];
        arguments.extend(other);
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}
