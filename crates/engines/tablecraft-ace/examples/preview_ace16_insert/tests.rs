use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Directory(pub(crate) PathBuf);

impl Directory {
    pub(crate) fn new() -> Self {
        let ordinal = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tablecraft-insert-preview-{}-{ordinal}",
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

#[test]
fn invalid_complete_input_is_rejected_before_source_open_or_output_creation() {
    let directory = Directory::new();
    let requests = directory.file("requests.tsv", b"int32\t1\tbinary\tzz\n");
    let output = directory.0.join("candidate.accdb");
    let args = Args::try_parse_from([
        "insert".as_ref(),
        directory.0.join("absent.accdb").as_os_str(),
        output.as_os_str(),
        "Items".as_ref(),
        requests.as_os_str(),
    ])
    .unwrap();
    let error = run(args, &mut budget()).unwrap_err();
    assert!(error.to_string().contains("invalid Binary hex"));
    assert!(!output.exists());
}

#[test]
fn value_byte_argument_keeps_the_default_and_a_closed_explicit_range() {
    let positional = [
        "insert",
        "source.accdb",
        "output.accdb",
        "Items",
        "rows.tsv",
    ];
    assert_eq!(Args::try_parse_from(positional).unwrap().value_bytes, 1024);
    for limit in [1, 1024, input::MAX_VALUE_BYTES] {
        let text = limit.to_string();
        let args = Args::try_parse_from([
            "insert",
            "--value-bytes",
            text.as_str(),
            "source.accdb",
            "output.accdb",
            "Items",
            "rows.tsv",
        ])
        .unwrap();
        assert_eq!(args.value_bytes, limit);
    }
    for invalid in [
        "0",
        "16777217",
        "-1",
        "1.5",
        "large",
        "18446744073709551616",
    ] {
        assert!(
            Args::try_parse_from([
                "insert",
                "--value-bytes",
                invalid,
                "source.accdb",
                "output.accdb",
                "Items",
                "rows.tsv",
            ])
            .is_err()
        );
    }
}

#[test]
fn direct_invalid_value_limits_refuse_before_request_or_database_file_io() {
    let directory = Directory::new();
    for value_bytes in [0, input::MAX_VALUE_BYTES + 1] {
        let output = directory.0.join("candidate.accdb");
        let args = Args {
            source: directory.0.join("absent.accdb"),
            output: output.clone(),
            table: "Items".into(),
            requests: directory.0.join("absent.tsv"),
            value_bytes,
        };
        let error = run(args, &mut budget()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Value-byte limit must be between")
        );
        assert!(!output.exists());
    }
}

#[test]
fn explicit_large_value_limit_reaches_the_source_only_after_complete_input_capture() {
    let directory = Directory::new();
    let encoded = format!("int32\t10\tbinary\t{}\n", "85".repeat(4077));
    let requests = directory.file("rows.tsv", encoded.as_bytes());
    let output = directory.0.join("candidate.accdb");
    let arguments = |value_bytes| Args {
        source: directory.0.join("absent.accdb"),
        output: output.clone(),
        table: "Items".into(),
        requests: requests.clone(),
        value_bytes,
    };
    let error = run(arguments(1024), &mut budget()).unwrap_err();
    assert!(error.to_string().contains("value exceeds input ceiling"));
    let error = run(arguments(8154), &mut budget()).unwrap_err();
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(!output.exists());
}
