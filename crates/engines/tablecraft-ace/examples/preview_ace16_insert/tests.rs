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
fn invalid_extended_date_refuses_before_database_open_or_output_creation() {
    let directory = Directory::new();
    let requests = directory.file("rows.tsv", b"extended-date\t1900-02-29T00:00:00.0000000\n");
    let output = directory.0.join("candidate.accdb");
    let args = Args {
        source: directory.0.join("absent.accdb"),
        output: output.clone(),
        table: "Items".into(),
        requests,
        value_bytes: 1024,
        incrementing_last: None,
    };
    let error = run(args, &mut budget()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("invalid extended-date civil input")
    );
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
            incrementing_last: None,
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
        incrementing_last: None,
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

#[test]
fn incrementing_flag_is_explicit_and_parses_an_exact_signed_counter() {
    let positional = [
        "insert",
        "source.accdb",
        "output.accdb",
        "Items",
        "rows.tsv",
    ];
    let manual = Args::try_parse_from(positional).unwrap();
    assert!(manual.incrementing_last.is_none());
    assert_eq!(manual.value_bytes, 1024);

    for last in [i32::MIN, 0, 3, i32::MAX] {
        let text = last.to_string();
        let args = Args::try_parse_from([
            "insert",
            "--incrementing-last",
            text.as_str(),
            "source.accdb",
            "output.accdb",
            "Items",
            "rows.tsv",
        ])
        .unwrap();
        assert_eq!(args.incrementing_last, Some(last));
        assert_eq!(args.value_bytes, 1024);
    }
    for invalid in ["", "1.0", "next", "2147483648", "-2147483649"] {
        assert!(
            Args::try_parse_from([
                "insert",
                "--incrementing-last",
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
fn later_malformed_companion_refuses_before_database_open_or_publication() {
    let directory = Directory::new();
    let requests = directory.file(
        "companions 'ユニコ$().tsv",
        b"int32\t10\tnull\t\nint32\t11\tbinary\tzz\n",
    );
    let output = directory.0.join("candidate.accdb");
    let args = Args {
        source: directory.0.join("absent.accdb"),
        output: output.clone(),
        table: "Items".into(),
        requests,
        value_bytes: 1024,
        incrementing_last: Some(3),
    };
    let error = run(args, &mut budget()).unwrap_err();
    assert!(error.to_string().contains("invalid Binary hex"));
    assert!(!output.exists());
}

#[test]
fn companion_value_limit_and_complete_capture_precede_database_open() {
    let directory = Directory::new();
    let encoded = format!("int32\t10\ttext\t\"{}\"\n", "x".repeat(1025));
    let requests = directory.file("companions.tsv", encoded.as_bytes());
    let output = directory.0.join("candidate.accdb");
    let arguments = |value_bytes| Args {
        source: directory.0.join("absent.accdb"),
        output: output.clone(),
        table: "Items".into(),
        requests: requests.clone(),
        value_bytes,
        incrementing_last: Some(3),
    };
    let error = run(arguments(1024), &mut budget()).unwrap_err();
    assert!(error.to_string().contains("value exceeds input ceiling"));
    let error = run(arguments(1025), &mut budget()).unwrap_err();
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(!output.exists());
}

#[test]
fn reports_keep_manual_protocol_and_generated_request_order() {
    let mut manual = Vec::new();
    write_report(
        &mut manual,
        Published {
            changed_rows: 2,
            generated: None,
        },
    )
    .unwrap();
    assert_eq!(manual, b"candidate_changed_rows=2\n");

    let mut generated = Vec::new();
    write_report(
        &mut generated,
        Published {
            changed_rows: 2,
            generated: Some(Generated {
                column: 1,
                values: vec![4, 5],
            }),
        },
    )
    .unwrap();
    assert_eq!(
        generated,
        b"candidate_changed_rows=2\ngenerated_column_number=1\ngenerated_value[0]=4\ngenerated_value[1]=5\n"
    );
}
