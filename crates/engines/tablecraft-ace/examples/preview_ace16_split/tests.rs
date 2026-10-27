use super::*;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_CASE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Directory(pub(crate) PathBuf);

impl Directory {
    pub(crate) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-split-consumer-{}-{}",
            std::process::id(),
            NEXT_CASE.fetch_add(1, Ordering::Relaxed),
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

#[test]
fn typed_arguments_accept_int32_bounds_and_reject_invalid_values_before_io() {
    let parsed = Args::try_parse_from([
        "preview",
        "missing-source",
        "fresh-output",
        "Items",
        "Score",
        "0",
        "-2147483648",
        "2147483647",
    ])
    .unwrap();
    assert_eq!(
        (parsed.row, parsed.expected, parsed.value),
        (0, i32::MIN, i32::MAX)
    );
    for (row, expected, value) in [
        ("-1", "1", "2"),
        ("0", "null", "2"),
        ("0", "1", "2147483648"),
    ] {
        assert!(
            Args::try_parse_from([
                "preview",
                "missing-source",
                "fresh-output",
                "Items",
                "Score",
                row,
                expected,
                value
            ])
            .is_err()
        );
    }
}

#[test]
fn real_split_candidate_is_fresh_synced_and_never_overwrites_source_or_output() {
    let directory = Directory::new();
    let source = directory.0.join("before.accdb");
    let output = directory.0.join("candidate.accdb");
    let before = model::source(6, false, true);
    fs::write(&source, &before).unwrap();
    let args = || Args {
        source: source.clone(),
        output: output.clone(),
        table: "Items".into(),
        column: "Score".into(),
        row: 0,
        expected: 1,
        value: i32::MAX,
    };
    assert_eq!(run(args(), &mut model::budget()).unwrap(), 1);
    let candidate = fs::read(&output).unwrap();
    assert_eq!(candidate.len(), before.len() + 2 * model::PAGE);
    model::assert_checked(&candidate);
    assert_eq!(fs::read(&source).unwrap(), before);
    assert!(run(args(), &mut model::budget()).is_err());
    assert_eq!(fs::read(&output).unwrap(), candidate);
    assert_eq!(fs::read(&source).unwrap(), before);
}
