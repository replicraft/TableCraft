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
fn closed_carriers_validate_before_source_io() {
    for (kind, expected, value, valid) in [
        ("byte", "0", "255", true),
        ("int16", "-32768", "32767", true),
        ("int64", "-9223372036854775808", "9223372036854775807", true),
        ("currency-scaled", "0", "9223372036854775807", true),
        ("byte", "0", "256", false),
        ("int32", "null", "1", false),
    ] {
        let args = Args::try_parse_from([
            "preview",
            "missing-source",
            "fresh-output",
            "Items",
            "Score",
            "0",
            kind,
            expected,
            value,
        ])
        .unwrap();
        assert_eq!(
            args.kind
                .parse(&args.expected)
                .and_then(|_| args.kind.parse(&args.value))
                .is_ok(),
            valid
        );
        if !valid {
            let error = run(args, &mut model::budget()).unwrap_err().to_string();
            assert!(error.contains("scalar input") || error.contains("NULL requests"));
        }
    }
    assert!(
        Args::try_parse_from([
            "preview",
            "missing",
            "fresh",
            "Items",
            "Score",
            "0",
            "double-bits",
            "0",
            "1",
        ])
        .is_err()
    );
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
        kind: Kind::Int32,
        expected: "1".into(),
        value: i32::MAX.to_string(),
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
