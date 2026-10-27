#[allow(dead_code)]
#[path = "../../tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../tests/ace16_long_value_updates_contract/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../../tests/support/mod.rs"]
mod support;

use super::*;
use std::{
    fs,
    io::Cursor,
    sync::atomic::{AtomicU64, Ordering},
};
use tablecraft_ace::{
    Ace16WritePreview, EditError, InspectError, SlotScalarUpdate, check_table,
    check_table_constraints, check_table_long_values, read_table,
};
use tablecraft_core::values::Value;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Directory(pub(crate) PathBuf);

impl Directory {
    pub(crate) fn new() -> Self {
        let ordinal = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tablecraft-long-value-consumer-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(crate) fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
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

fn request(directory: &Directory, name: &str, kind: &str, value: &str) -> PathBuf {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_writer(Vec::new());
    writer
        .write_record(["0", "text", "A", kind, value])
        .unwrap();
    directory.file(name, &writer.into_inner().unwrap())
}

#[test]
fn real_chained_memo_candidate_is_complete_fresh_and_preserves_source_and_old_chunks() {
    let directory = Directory::new();
    let source = model::source(6, 1);
    model::check(&source);
    let source_path = directory.file("source.accdb", &source);
    let output = directory.0.join("candidate.accdb");
    let replacement = "Ā".repeat(5000);
    let requests = request(&directory, "requests.tsv", "text", &replacement);
    let args = || Args {
        source: source_path.clone(),
        output: output.clone(),
        table: "Items".into(),
        column: "Memo".into(),
        requests: requests.clone(),
    };

    assert_eq!(run(args(), &mut budget()).unwrap(), 1);
    let candidate = fs::read(&output).unwrap();
    model::check(&candidate);
    assert_eq!(model::read(&candidate)[0][1], Value::Text(replacement));
    model::old_chunks_unchanged(&source, &candidate);
    assert_eq!(fs::read(&source_path).unwrap(), source);

    assert!(run(args(), &mut budget()).is_err());
    assert_eq!(fs::read(&output).unwrap(), candidate);
    assert_eq!(fs::read(&source_path).unwrap(), source);
}

#[test]
fn long_value_input_has_explicit_text_and_hex_bounds_without_widening_slot_profile() {
    let directory = Directory::new();
    let text = "x".repeat(65_536);
    let text_path = request(&directory, "text.tsv", "text", &text);
    let rows = input::load(&text_path, &mut budget(), REQUEST_VALUE_BYTES).unwrap();
    assert_eq!(rows[0].value(), &Value::Text(text));
    assert!(input::load(&text_path, &mut budget(), 1024).is_err());

    let binary = "aB".repeat(32_768);
    let binary_path = request(&directory, "binary.tsv", "binary", &binary);
    let rows = input::load(&binary_path, &mut budget(), REQUEST_VALUE_BYTES).unwrap();
    assert_eq!(rows[0].value(), &Value::Binary(vec![0xab; 32_768]));

    let excessive = "x".repeat(65_537);
    let excessive_path = request(&directory, "excessive.tsv", "text", &excessive);
    assert!(input::load(&excessive_path, &mut budget(), REQUEST_VALUE_BYTES).is_err());
}

#[test]
fn late_invalid_request_refuses_before_database_open_or_output_publication() {
    let directory = Directory::new();
    let output = directory.file("protected-output", b"keep");
    let requests = directory.file(
        "invalid.tsv",
        b"0\ttext\tA\ttext\tB\n1\tbinary\tzz\ttext\tC\n",
    );
    let args = Args {
        source: directory.0.join("missing-database"),
        output: output.clone(),
        table: "Items".into(),
        column: "Memo".into(),
        requests,
    };

    let error = run(args, &mut budget()).unwrap_err();
    assert!(error.to_string().contains("invalid Binary hex"));
    assert_eq!(fs::read(output).unwrap(), b"keep");
}
