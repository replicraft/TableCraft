#[allow(dead_code)]
#[path = "../../tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../tests/mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "../../tests/ace16_encoded_delete_contract/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../../tests/ace16_page_updates_contract/model.rs"]
mod page_model;
#[allow(dead_code)]
#[path = "../../tests/support/mod.rs"]
mod support;
#[allow(dead_code)]
#[path = "../../tests/ace16_variable_updates_contract/model.rs"]
mod variable_model;

use super::*;
use page_model::capture;
use std::{
    fs,
    io::Cursor,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_ace::{check_table, read_table};
use tablecraft_core::values::Value;

const SCORE_WRITABLE_FLAGS_OFFSET: usize = 10 * 4096 + 115;
static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Directory(pub(crate) PathBuf);

impl Directory {
    pub(crate) fn new() -> Self {
        let ordinal = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tablecraft-slot-preview-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(crate) fn file(&self, name: &str, contents: &[u8]) -> PathBuf {
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

pub(crate) fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET)
}

fn arguments(source: &Path, output: &Path, requests: &Path) -> Args {
    Args::try_parse_from([
        "slot".as_ref(),
        source.as_os_str(),
        output.as_os_str(),
        "Items".as_ref(),
        "Score".as_ref(),
        requests.as_os_str(),
    ])
    .unwrap()
}

#[test]
fn clap_requires_exact_sources_and_explicit_exclusive_modes() {
    for invalid in [
        vec!["slot", "before", "after", "Items", "Score"],
        vec![
            "slot", "before", "after", "Items", "Score", "one.tsv", "extra",
        ],
        vec![
            "slot", "before", "after", "Items", "Score", "--delete", "0:ff",
        ],
    ] {
        assert!(Args::try_parse_from(invalid).is_err());
    }
    let args =
        Args::try_parse_from(["slot", "before", "after", "Items", "Score", "one.tsv"]).unwrap();
    assert_eq!(args.requests, PathBuf::from("one.tsv"));
    assert!(!args.page_updates);
    assert!(!args.variable_updates);

    let args = Args::try_parse_from([
        "slot",
        "before",
        "after",
        "Items",
        "Score",
        "one.tsv",
        "--page-updates",
    ])
    .unwrap();
    assert!(args.page_updates);
    assert!(!args.variable_updates);

    let args = Args::try_parse_from([
        "slot",
        "before",
        "after",
        "Items",
        "Score",
        "one.tsv",
        "--variable-updates",
    ])
    .unwrap();
    assert!(args.variable_updates);
    assert!(!args.page_updates);
    assert!(
        Args::try_parse_from([
            "slot",
            "before",
            "after",
            "Items",
            "Score",
            "one.tsv",
            "--page-updates",
            "--variable-updates",
        ])
        .is_err()
    );
}

#[test]
fn malformed_later_assignment_fails_before_missing_database_or_output() {
    let directory = Directory::new();
    let requests = directory.file(
        "bad.tsv",
        b"2\ttext\ta\ttext\tb\n3\tbinary\tzz\tbinary\t00\n",
    );
    let source = directory.0.join("missing.accdb");
    let output = directory.0.join("candidate.accdb");
    let error = run(arguments(&source, &output, &requests), &mut budget())
        .err()
        .unwrap();
    assert!(error.to_string().contains("Invalid slot request 2"));
    assert!(!source.exists());
    assert!(!output.exists());
}

#[test]
fn real_api_candidate_is_fresh_readable_and_never_overwrites_original_or_existing_output() {
    let directory = Directory::new();
    let mut model = model::text(1, false, true);
    // Independent literal metadata wrapper: native writable Short Text flag 2.
    model.bytes[SCORE_WRITABLE_FLAGS_OFFSET] = 2;
    let source = directory.file("before.accdb", &model.bytes);
    let requests = directory.file("one.tsv", "2\ttext\ta\ttext\té\n".as_bytes());
    let output = directory.0.join("candidate.accdb");
    assert_eq!(
        run(arguments(&source, &output, &requests), &mut budget()).unwrap(),
        1
    );
    let after = fs::read(&output).unwrap();
    assert_ne!(after, model.bytes);
    assert_eq!(after.len(), model.bytes.len());
    let snapshot = read_table(
        &mut Cursor::new(&after),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let selected = snapshot
        .rows()
        .iter()
        .find(|row| row[0] == Value::Int32(3))
        .unwrap();
    assert_eq!(selected[1], Value::Text("é".into()));
    assert_eq!(snapshot.rows().len(), model.records.len());
    check_table(
        &mut Cursor::new(&after),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(run(arguments(&source, &output, &requests), &mut budget()).is_err());
    assert_eq!(fs::read(&output).unwrap(), after);
    assert!(run(arguments(&source, &source, &requests), &mut budget()).is_err());
    assert_eq!(fs::read(&source).unwrap(), model.bytes);
}

#[test]
fn page_mode_expands_only_when_selected_and_preserves_fresh_output() {
    let directory = Directory::new();
    let before = page_model::image(&[(0, page_model::record(b"A"))], 1, true);
    assert_eq!(page_model::read(&before).len(), 1);
    let source = directory.file("before-page.accdb", &before);
    let requests = directory.file("growth.tsv", b"0\tbinary\t41\tbinary\t4142434445\n");
    let output = directory.0.join("page-candidate.accdb");

    let error = run(arguments(&source, &output, &requests), &mut budget())
        .err()
        .unwrap();
    assert_eq!(
        error.downcast_ref::<tablecraft_ace::EditError>(),
        Some(&tablecraft_ace::EditError::Unsupported(
            tablecraft_ace::WriteBlocker::RowLayout
        ))
    );
    assert!(!output.exists());

    let mut args = arguments(&source, &output, &requests);
    args.page_updates = true;
    assert_eq!(run(args, &mut budget()).unwrap(), 1);
    let after = fs::read(&output).unwrap();
    assert_eq!(after.len(), before.len());
    assert_eq!(
        page_model::read(&after)[0][1],
        Value::Binary(b"ABCDE".to_vec())
    );
    assert_eq!(page_model::count(&after), 2);
    assert_eq!(fs::read(&source).unwrap(), before);

    let mut args = arguments(&source, &output, &requests);
    args.page_updates = true;
    let error = run(args, &mut budget()).err().unwrap();
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&output).unwrap(), after);
    assert_eq!(fs::read(&source).unwrap(), before);
}

#[test]
fn variable_mode_allocates_only_when_explicitly_selected() {
    let directory = Directory::new();
    let before = variable_model::source(false, false);
    let source = directory.file("before-variable.accdb", &before);
    let replacement = vec![0x5b; 255];
    let encoded = "5b".repeat(255);
    let requests = directory.file(
        "allocation.tsv",
        format!("0\tbinary\t41\tbinary\t{encoded}\n").as_bytes(),
    );
    let output = directory.0.join("variable-candidate.accdb");

    let mut args = arguments(&source, &output, &requests);
    args.page_updates = true;
    let error = run(args, &mut budget()).err().unwrap();
    assert_eq!(
        error.downcast_ref::<tablecraft_ace::EditError>(),
        Some(&tablecraft_ace::EditError::Unsupported(
            tablecraft_ace::WriteBlocker::RowLayout
        ))
    );
    assert!(!output.exists());

    let mut args = arguments(&source, &output, &requests);
    args.variable_updates = true;
    assert_eq!(run(args, &mut budget()).unwrap(), 1);
    let after = fs::read(&output).unwrap();
    assert_eq!(after.len(), before.len() + 4096);
    assert_eq!(page_model::read(&after)[0][1], Value::Binary(replacement));
    assert_eq!(fs::read(&source).unwrap(), before);
}
