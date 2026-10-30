#[allow(dead_code)]
#[path = "../../tests/ace16_encoded_delete_contract/model.rs"]
mod encoded_model;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "../../tests/mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "../../tests/ace16_encoded_split_contract/model.rs"]
mod split_model;
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

const PAGE_BYTES: usize = 4096;
const DATA_PAGES: [usize; 5] = [12, 15, 16, 17, 18];
const DEFINITION_HEADER: usize = 63;
const COLUMN_COUNT_OFFSET: usize = 45;
const LOGICAL_COUNT_OFFSET: usize = 47;
const PHYSICAL_COUNT_OFFSET: usize = 51;
const COUNTER_BYTES: usize = 12;
const COLUMN_BYTES: usize = 25;
const PHYSICAL_BYTES: usize = 52;
const LOGICAL_BYTES: usize = 28;
static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Directory(pub(crate) PathBuf);

impl Directory {
    pub(crate) fn new() -> Self {
        let ordinal = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tablecraft-variable-split-preview-{}-{ordinal}",
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
        "variable-split".as_ref(),
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
        vec!["variable-split", "before", "after", "Items", "Score"],
        vec![
            "variable-split",
            "before",
            "after",
            "Items",
            "Score",
            "one.tsv",
            "extra",
        ],
        vec![
            "variable-split",
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
    let args = Args::try_parse_from([
        "variable-split",
        "before",
        "after",
        "Items",
        "Score",
        "one.tsv",
    ])
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
    let original = variable_source();
    let source = directory.file("before.accdb", &original);
    let text = format!("0\tbinary\t00\tbinary\t{}\n", "ff".repeat(255));
    let requests = directory.file("one.tsv", text.as_bytes());
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
    assert_eq!(table.rows()[0][1], Value::Binary(vec![255; 255]));
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
    assert_eq!(fs::read(&source).unwrap(), original);

    assert!(run(arguments(&source, &output, &requests), &mut budget()).is_err());
    assert_eq!(fs::read(&output).unwrap(), after);
    assert!(run(arguments(&source, &source, &requests), &mut budget()).is_err());
    assert_eq!(fs::read(&source).unwrap(), original);
}

fn word(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().unwrap(),
    ))
}

fn declaration_end(bytes: &[u8], definition: usize) -> usize {
    let base = definition * PAGE_BYTES;
    let columns = word(bytes, base + COLUMN_COUNT_OFFSET);
    let physical = word(bytes, base + PHYSICAL_COUNT_OFFSET);
    let logical = word(bytes, base + LOGICAL_COUNT_OFFSET);
    let mut position = base + DEFINITION_HEADER + physical * COUNTER_BYTES + columns * COLUMN_BYTES;
    for _ in 0..columns {
        position += 2 + word(bytes, position);
    }
    position += physical * PHYSICAL_BYTES + logical * LOGICAL_BYTES;
    for _ in 0..logical {
        position += 2 + word(bytes, position);
    }
    assert!(position + 2 <= base + PAGE_BYTES);
    position
}

fn variable_source() -> Vec<u8> {
    let mut original = split_model::source(split_model::Family::Binary, false, true).bytes;
    support::put32(&mut original, 10 * PAGE_BYTES + 59, (11 << 8) | 1);
    let mut owned = vec![0; 69];
    for number in DATA_PAGES {
        owned[5 + number / 8] |= 1 << (number % 8);
        let base = number * PAGE_BYTES;
        let count = word(&original, base + 12);
        let last = word(&original, base + 14 + 2 * (count - 1)) & 0x1fff;
        let free = last - (14 + 2 * count);
        support::put16(&mut original, base + 2, free as u16);
    }
    let mut hint = vec![0; 69];
    hint[5 + 18 / 8] |= 1 << (18 % 8);
    original[11 * PAGE_BYTES..12 * PAGE_BYTES]
        .copy_from_slice(&support::data_page(0, &[(0, owned), (0, hint)]));

    // These literals have no LVAL declarations. Record their complete empty
    // tails explicitly instead of treating unqualified zero suffixes as proof.
    for definition in [2, 7, 10] {
        let end = declaration_end(&original, definition);
        support::put16(&mut original, end, 0xffff);
    }
    original
}
