#[allow(dead_code)]
#[path = "../../tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../tests/ace16_restrict_delete_contract/model.rs"]
mod model;
use tablecraft_testkit::access as support;

use super::*;
use std::{
    fmt::Write as _,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_ace::{InspectError, RowDelete};

const REQUEST_FILE_BYTES: u64 = 32 * 1024 * 1024;
const REQUEST_FILE_ROWS: usize = 4096;
static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let ordinal = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tablecraft-batch-delete-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET)
}

fn pairs(groups: &[(&str, &Path)]) -> Vec<OsString> {
    let mut result = Vec::new();
    for &(table, path) in groups {
        result.push(table.into());
        result.push(path.as_os_str().to_owned());
    }
    result
}

fn arguments(source: &Path, output: &Path, groups: &[(&str, &Path)]) -> Args {
    let mut arguments = vec![
        OsString::from("batch-delete"),
        source.as_os_str().to_owned(),
        output.as_os_str().to_owned(),
    ];
    for &(table, path) in groups {
        arguments.push("--group".into());
        arguments.push(table.into());
        arguments.push(path.as_os_str().to_owned());
    }
    Args::try_parse_from(arguments).unwrap()
}

#[test]
fn clap_requires_complete_repeated_pairs_and_keeps_single_example_switches_separate() {
    for invalid in [
        vec!["batch", "source", "output"],
        vec!["batch", "source", "output", "--group", "Items"],
        vec!["batch", "source", "output", "--delete", "0:0200"],
        vec!["batch", "source", "output", "--delete-file", "one.tsv"],
    ] {
        assert!(Args::try_parse_from(invalid).is_err());
    }
    let args = Args::try_parse_from([
        "batch",
        "source",
        "output",
        "--group",
        "Child Table",
        "one.tsv",
        "--group",
        "Parents",
        "two.tsv",
    ])
    .unwrap();
    assert_eq!(
        args.groups,
        ["Child Table", "one.tsv", "Parents", "two.tsv"].map(OsString::from)
    );
}

#[test]
fn all_files_reuse_exact_quoted_tsv_capture_and_repeated_table_groups_refuse() {
    let directory = Directory::new();
    let first = directory.file("first.tsv", b"\"0\"\t\"0200aB\"\n");
    let second = directory.file("second.tsv", b"3\t0300FE\n");
    let loaded = input::load(
        pairs(&[("Child Table", &first), ("Parents", &second)]),
        &mut budget(),
    )
    .unwrap();
    assert_eq!(loaded[0].table, "Child Table");
    assert_eq!(loaded[0].deletes[0].row(), 0);
    assert_eq!(loaded[0].deletes[0].expected_bytes(), [2, 0, 171]);
    assert_eq!(loaded[1].deletes[0].row(), 3);
    assert_eq!(loaded[1].deletes[0].expected_bytes(), [3, 0, 254]);
    let duplicate = input::load(
        pairs(&[("Items", &first), ("ITEMS", &second)]),
        &mut budget(),
    );
    assert!(
        duplicate
            .err()
            .unwrap()
            .to_string()
            .contains("Repeated table")
    );
}

#[test]
fn malformed_later_file_fails_before_database_open_or_output_creation() {
    let directory = Directory::new();
    let first = directory.file("first.tsv", b"0\t0200\n");
    let invalid = directory.file("invalid.tsv", b"0\t0200\n1\tzz\n");
    let missing_source = directory.0.join("missing.accdb");
    let output = directory.0.join("candidate.accdb");
    let args = arguments(
        &missing_source,
        &output,
        &[("Items", &first), ("Parents", &invalid)],
    );
    let error = run(args, &mut budget()).err().unwrap();
    assert!(
        error
            .to_string()
            .contains("Invalid delete-request record 2")
    );
    assert!(!missing_source.exists());
    assert!(!output.exists());
}

#[test]
fn per_file_and_cumulative_group_request_limits_are_distinct() {
    let directory = Directory::new();
    let sparse = directory.file("large.tsv", b"");
    File::options()
        .write(true)
        .open(&sparse)
        .unwrap()
        .set_len(REQUEST_FILE_BYTES + 1)
        .unwrap();
    let error = input::load(pairs(&[("Items", &sparse)]), &mut budget())
        .err()
        .unwrap();
    assert!(error.to_string().contains("32 MiB"));
    let too_many = directory.file(
        "many.tsv",
        "0\t0200\n".repeat(REQUEST_FILE_ROWS + 1).as_bytes(),
    );
    let error = input::load(pairs(&[("Items", &too_many)]), &mut budget())
        .err()
        .unwrap();
    assert!(error.to_string().contains("4096-row"));
    let too_wide = directory.file("wide.tsv", format!("0\t{}\n", "00".repeat(4097)).as_bytes());
    assert!(input::load(pairs(&[("Items", &too_wide)]), &mut budget()).is_err());

    let valid = directory.file("valid.tsv", b"0\t0200\n");
    let mut arguments = Vec::new();
    for index in 0..=NativeImage::MAX_DELETE_BATCH_TABLES {
        arguments.push(OsString::from(format!("Table{index}")));
        arguments.push(valid.as_os_str().to_owned());
    }
    let error = input::load(arguments, &mut budget()).err().unwrap();
    assert_eq!(
        error.downcast_ref::<InspectError>(),
        Some(&InspectError::StructureLimit)
    );

    let full = directory.file("full.tsv", "0\t0200\n".repeat(REQUEST_FILE_ROWS).as_bytes());
    let count = NativeImage::MAX_DELETE_BATCH_ROWS / REQUEST_FILE_ROWS + 1;
    let mut arguments = Vec::new();
    for index in 0..count {
        arguments.push(OsString::from(format!("Table{index}")));
        arguments.push(full.as_os_str().to_owned());
    }
    let error = input::load(arguments, &mut budget()).err().unwrap();
    assert_eq!(
        error.downcast_ref::<InspectError>(),
        Some(&InspectError::StructureLimit)
    );
}

#[test]
fn files_share_one_operation_budget_instead_of_resetting_each_group() {
    let directory = Directory::new();
    let first = directory.file("one.tsv", b"0\t0200\n");
    let second = directory.file("two.tsv", b"1\t0200\n");
    for file in [&first, &second] {
        let mut one_row = ResourceBudget::new(BYTE_BUDGET, 1, STEP_BUDGET);
        assert!(input::load(pairs(&[("Items", file)]), &mut one_row).is_ok());
    }
    let mut one_row = ResourceBudget::new(BYTE_BUDGET, 1, STEP_BUDGET);
    assert!(
        input::load(
            pairs(&[("Items", &first), ("Parents", &second)]),
            &mut one_row
        )
        .is_err()
    );
    for mut allowance in [
        ResourceBudget::new(0, ROW_BUDGET, STEP_BUDGET),
        ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, 0),
    ] {
        assert!(input::load(pairs(&[("Items", &first)]), &mut allowance).is_err());
    }
}

fn tsv(requests: &[RowDelete]) -> Vec<u8> {
    let mut output = String::new();
    for request in requests {
        write!(output, "{}\t", request.row()).unwrap();
        for byte in request.expected_bytes() {
            write!(output, "{byte:02x}").unwrap();
        }
        output.push('\n');
    }
    output.into_bytes()
}

#[test]
fn complete_candidate_publication_is_fresh_and_preserves_the_original_source() {
    let directory = Directory::new();
    let model = model::database(6, false, false, false);
    let source = directory.file("before.accdb", &model.bytes);
    let children = directory.file("children.tsv", &tsv(&model::requests(&model, 0, &[0, 1])));
    let parents = directory.file("parents.tsv", &tsv(&model::requests(&model, 1, &[0])));
    let output = directory.0.join("candidate.accdb");
    let groups = [
        ("Items", children.as_path()),
        ("Parents", parents.as_path()),
    ];
    assert_eq!(
        run(arguments(&source, &output, &groups), &mut budget()).unwrap(),
        3
    );
    assert_eq!(fs::read(&source).unwrap(), model.bytes);
    let after = fs::read(&output).unwrap();
    assert_ne!(after, model.bytes);
    assert_eq!(after.len(), model.bytes.len());
    assert!(run(arguments(&source, &output, &groups), &mut budget()).is_err());
    assert_eq!(fs::read(&output).unwrap(), after);
    assert_eq!(fs::read(&source).unwrap(), model.bytes);
}

#[path = "tests/cascades.rs"]
mod cascades;
