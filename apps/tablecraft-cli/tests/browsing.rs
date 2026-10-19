#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct FileCopy {
    path: PathBuf,
    original: Vec<u8>,
}
impl FileCopy {
    fn new(bytes: Vec<u8>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-browse-{}-{}.mdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, &bytes).unwrap();
        Self {
            path,
            original: bytes,
        }
    }
    fn run(&self, command: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .arg(command)
            .arg(&self.path)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for FileCopy {
    fn drop(&mut self) {
        assert_eq!(fs::read(&self.path).unwrap(), self.original);
        fs::remove_file(&self.path).unwrap();
    }
}
fn success(result: Output) -> Vec<u8> {
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    result.stdout
}
#[test]
fn jet4_discovery_rows_pagination_and_empty_pages_preserve_source() {
    let file = FileCopy::new(fixture::database());
    let tables = String::from_utf8(success(file.run("tables", &[]))).unwrap();
    assert!(tables.starts_with("Name\n"));
    assert!(tables.lines().any(|name| name == "Items"));
    assert!(tables.lines().any(|name| name == "MSysObjects"));
    assert_eq!(
        success(file.run("rows", &["Items"])),
        b"Id\tScore\n1\t10\n2\t20\n3\t30\n"
    );
    assert_eq!(
        success(file.run("rows", &["items", "--limit", "1", "--offset", "1"])),
        b"Id\tScore\n2\t20\n"
    );
    for offset in ["3", "4294967295"] {
        assert_eq!(
            success(file.run("rows", &["Items", "--offset", offset])),
            b"Id\tScore\n"
        );
    }
}
#[test]
fn real_sample_table_contents_match_existing_query_display_without_modification() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let file = FileCopy::new(fs::read(path).unwrap());
    let result = success(file.run("rows", &["ROULETTE_TB", "--limit", "2"]));
    let query = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "run",
            "SELECT TOP 2 * FROM ROULETTE_TB",
            "--database",
        ])
        .arg(&file.path)
        .output()
        .unwrap();
    assert_eq!(result, success(query));
    assert_eq!(String::from_utf8(result).unwrap().lines().count(), 3);
}
#[test]
fn invalid_ranges_names_and_truncated_files_publish_no_partial_output() {
    let file = FileCopy::new(fixture::database());
    for args in [
        vec!["Missing"],
        vec!["Items", "--limit", "0"],
        vec!["Items", "--limit", "16385"],
        vec!["Items", "--offset", "-1"],
        vec!["Items", "--offset", "4294967296"],
    ] {
        let result = file.run("rows", &args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    let truncated = FileCopy::new(file.original[..100].to_vec());
    for (command, args) in [("tables", vec![]), ("rows", vec!["Items"])] {
        let result = truncated.run(command, &args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn table_names_are_passed_as_data_and_unsupported_columns_do_not_get_hidden() {
    let mut bytes = fixture::database();
    let at = bytes
        .windows(b"Items".len())
        .position(|part| part == b"Items")
        .unwrap();
    bytes[at..at + 5].copy_from_slice(b"I ]x'");
    let file = FileCopy::new(bytes);
    assert_eq!(
        success(file.run("rows", &["I ]x'", "--limit", "1"])),
        b"Id\tScore\n1\t10\n"
    );
    let mut unsupported = fixture::database();
    // The authored fixture's first column descriptor in the table definition.
    const DEFINITION_PAGE: usize = 10;
    const PAGE_BYTES: usize = 4096;
    const FIRST_COLUMN_OFFSET: usize = 63;
    unsupported[DEFINITION_PAGE * PAGE_BYTES + FIRST_COLUMN_OFFSET] = u8::MAX;
    let file = FileCopy::new(unsupported);
    let result = file.run("rows", &["Items", "--limit", "1"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
