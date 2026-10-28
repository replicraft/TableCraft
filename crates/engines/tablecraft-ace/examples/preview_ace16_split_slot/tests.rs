#[allow(dead_code)]
#[path = "../../tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../tests/mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "../../tests/ace16_encoded_delete_contract/model.rs"]
mod model;
use tablecraft_testkit::access as support;

use super::*;
use std::{
    fs,
    io::Cursor,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_ace::{IndexCheckStatus, check_table, read_table};
use tablecraft_core::values::Value;

const SCORE_FLAGS_OFFSET: usize = 10 * 4096 + 115;
static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Directory(pub(crate) PathBuf);

impl Directory {
    pub(crate) fn new() -> Self {
        let ordinal = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tablecraft-split-slot-preview-{}-{ordinal}",
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
        "split-slot".as_ref(),
        source.as_os_str(),
        output.as_os_str(),
        "Items".as_ref(),
        "Score".as_ref(),
        requests.as_os_str(),
    ])
    .unwrap()
}

#[test]
fn clap_requires_five_positionals_without_unrelated_placement_modes() {
    for invalid in [
        vec!["split-slot", "before", "after", "Items", "Score"],
        vec![
            "split-slot",
            "before",
            "after",
            "Items",
            "Score",
            "one.tsv",
            "extra",
        ],
        vec![
            "split-slot",
            "before",
            "after",
            "Items",
            "Score",
            "one.tsv",
            "--variable-updates",
        ],
    ] {
        assert!(Args::try_parse_from(invalid).is_err());
    }
    let args = Args::try_parse_from(["split-slot", "before", "after", "Items", "Score", "one.tsv"])
        .unwrap();
    assert_eq!(args.requests, PathBuf::from("one.tsv"));
}

#[test]
fn malformed_later_tsv_is_rejected_before_missing_database_or_output() {
    let directory = Directory::new();
    let requests = directory.file(
        "bad.tsv",
        b"0\ttext\ta\ttext\tb\n1\tbinary\t+1\tbinary\t00\n",
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
fn real_split_api_candidate_is_readable_fresh_and_preserves_original() {
    let directory = Directory::new();
    let mut original = model::text(1, false, true);
    original.bytes[SCORE_FLAGS_OFFSET] = 2;
    let source = directory.file("before.accdb", &original.bytes);
    let requests = directory.file("one.tsv", "2\ttext\ta\ttext\té\n".as_bytes());
    let output = directory.0.join("candidate.accdb");
    assert_eq!(
        run(arguments(&source, &output, &requests), &mut budget()).unwrap(),
        1
    );
    let after = fs::read(&output).unwrap();
    let table = read_table(
        &mut Cursor::new(&after),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows()[2][1], Value::Text("é".into()));
    let checked = check_table(
        &mut Cursor::new(&after),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert_eq!(fs::read(&source).unwrap(), original.bytes);

    assert!(run(arguments(&source, &output, &requests), &mut budget()).is_err());
    assert_eq!(fs::read(&output).unwrap(), after);
    assert!(run(arguments(&source, &source, &requests), &mut budget()).is_err());
    assert_eq!(fs::read(&source).unwrap(), original.bytes);
}
