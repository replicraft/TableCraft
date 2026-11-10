//! Transport sizes come from actual inputs and explicit caller budgets.
use super::{Cli, Error, Parser, command, input, updates};
use crate::commands::{
    tsv,
    write::{args::Command, run},
};
use std::{fmt::Write, fs};
use tablecraft_core::{limits::ResourceBudget, values::Value};

#[test]
fn quoted_record_measurements_keep_embedded_delimiters_newlines_and_escaped_quotes() {
    let record = b"\"a\tb\"\t\"first\n\"\"quoted\"\"\r\nlast\"\r\n";
    let mut bytes = b"Name\tValue\r\n".to_vec();
    bytes.extend(record);
    bytes.extend(b"x\ty");
    let layout = tsv::validate(&bytes, &mut ResourceBudget::new(0, 0, 100_000), &|| false).unwrap();
    assert_eq!(layout.max_record_bytes, record.len());
    assert_eq!(layout.max_fields, 2);
    assert!(tsv::validate(&bytes, &mut ResourceBudget::new(0, 0, 100_000), &|| true).is_err());
}

#[test]
fn more_than_4096_quoted_updates_fit_paid_actual_record_storage() {
    let mut bytes = String::new();
    for ordinal in 0..5000 {
        writeln!(
            bytes,
            "Items\tMemo\t{ordinal}\ttext\t\"old\tline\none\"\t\"new\tline\none\""
        )
        .unwrap();
    }
    let mut work = ResourceBudget::new(4 * 1024 * 1024, 10_000, 100_000_000);
    let cells = input::update_file::cells(bytes.as_bytes(), &mut work, &|| false).unwrap();
    let groups = updates::parse(
        &cells,
        usize::MAX,
        |row, expected, value| (row, expected, value),
        &mut work,
        &|| false,
    )
    .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].rows.len(), 5000);
    assert_eq!(
        groups[0].rows[4999],
        (
            4999,
            Value::Text("old\tline\none".into()),
            Value::Text("new\tline\none".into())
        )
    );

    let directory = tempfile::tempdir().unwrap();
    let requests = directory.path().join("late-invalid.tsv");
    bytes.push_str("Items\tMemo\t5000\ttext\tnull\t\"unterminated");
    fs::write(&requests, bytes).unwrap();
    let source = directory.path().join("missing.accdb");
    let output = directory.path().join("unpublished.accdb");
    let args = command(&[
        "tablecraft",
        "write",
        "preview-update",
        source.to_str().unwrap(),
        output.to_str().unwrap(),
        "--set-file",
        requests.to_str().unwrap(),
    ]);
    assert!(matches!(run::run(args, None), Err(Error::Input(_))));
    assert!(!source.exists());
    assert!(!output.exists());
}

#[test]
fn request_capture_above_16_mebibytes_is_exact_and_lower_explicit_budget_refuses() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large.tsv");
    let expected = vec![b'x'; 16 * 1024 * 1024 + 1];
    fs::write(&path, &expected).unwrap();
    let captured = input::capture(
        &path,
        &mut ResourceBudget::new(32 * 1024 * 1024, 0, 100_000),
        &|| false,
    )
    .unwrap();
    assert!(captured == expected);
    assert!(matches!(
        input::capture(
            &path,
            &mut ResourceBudget::new(16 * 1024 * 1024, 0, 100_000),
            &|| false
        ),
        Err(Error::Native(tablecraft_ace::InspectError::Limit(_)))
    ));
    assert!(
        input::capture(
            &path,
            &mut ResourceBudget::new(32 * 1024 * 1024, 0, 100_000),
            &|| true
        )
        .is_err()
    );
}

#[test]
fn value_byte_limit_is_optional_and_explicit_positive_values_have_no_transport_maximum() {
    let Command::Insert(args) = command(&[
        "tablecraft",
        "write",
        "preview-insert",
        "source.accdb",
        "out.accdb",
        "Items",
        "rows.tsv",
    ]) else {
        panic!("insert");
    };
    assert_eq!(args.value_bytes, None);
    let bytes = format!("text\t{}\n", "x".repeat(2048));
    let rows = input::presence::rows(
        bytes.as_bytes(),
        args.value_bytes.unwrap_or(usize::MAX),
        &mut ResourceBudget::new(100_000, 100, 100_000),
        &|| false,
    )
    .unwrap();
    assert!(
        matches!(&rows[0][0], tablecraft_ace::InsertFieldValue::Supplied(Value::Text(value)) if value.len() == 2048)
    );
    let maximum = usize::MAX.to_string();
    let Command::Insert(args) = command(&[
        "tablecraft",
        "write",
        "preview-insert",
        "source.accdb",
        "out.accdb",
        "Items",
        "rows.tsv",
        "--value-bytes",
        &maximum,
    ]) else {
        panic!("insert");
    };
    assert_eq!(args.value_bytes, Some(usize::MAX));
    assert!(
        Cli::try_parse_from([
            "tablecraft",
            "write",
            "preview-insert",
            "source.accdb",
            "out.accdb",
            "Items",
            "rows.tsv",
            "--value-bytes",
            "0"
        ])
        .is_err()
    );
    assert!(
        input::presence::rows(
            bytes.as_bytes(),
            1024,
            &mut ResourceBudget::new(100_000, 100, 100_000),
            &|| false
        )
        .is_err()
    );
}
