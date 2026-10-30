mod mixed;

use super::{args::Command, error::Error, input, updates, values};
use crate::{Cli, commands};
use clap::Parser;
use tablecraft_ace::{FixedScalarUpdate, RowInsert};
use tablecraft_core::{limits::ResourceBudget, values::Value};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 100_000_000)
}

fn command(arguments: &[&str]) -> Command {
    let args = Cli::try_parse_from(arguments).unwrap();
    match args.command.unwrap() {
        commands::Command::Write { command } => command,
        _ => panic!("expected write command"),
    }
}

#[test]
fn explicit_modes_and_conflicts_are_checked_before_database_open() {
    for arguments in [
        vec![
            "tablecraft",
            "write",
            "preview-update",
            "missing.accdb",
            "out.accdb",
            "--set",
            "Contacts",
            "Name",
            "0",
            "text",
            "Alice",
            "Bob",
            "--variable",
            "--index-growth",
        ],
        vec![
            "tablecraft",
            "write",
            "preview-delete",
            "missing.accdb",
            "out.accdb",
            "Contacts",
            "--row",
            "1",
        ],
        vec![
            "tablecraft",
            "write",
            "preview-insert",
            "missing.accdb",
            "out.accdb",
            "Contacts",
            "input.tsv",
            "--incrementing-last",
            "-2",
        ],
    ] {
        command(&arguments);
    }
    for flags in [
        vec!["--index-growth", "--cascade-updates"],
        vec!["--variable", "--long-value"],
        vec!["--long-value", "--index-growth"],
    ] {
        let mut arguments = vec![
            "tablecraft",
            "write",
            "preview-update",
            "missing.accdb",
            "out.accdb",
            "--set",
            "Items",
            "Score",
            "0",
            "int32",
            "10",
            "11",
        ];
        arguments.extend(flags);
        assert!(Cli::try_parse_from(arguments).is_err());
    }
    assert!(
        Cli::try_parse_from([
            "tablecraft",
            "write",
            "preview-delete",
            "missing.accdb",
            "out.accdb",
            "Items",
            "--row",
            "0",
            "--delete",
            "0:0000"
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "tablecraft",
            "write",
            "preview-delete",
            "missing.accdb",
            "out.accdb",
            "Items"
        ])
        .is_err()
    );
    let malformed = command(&[
        "tablecraft",
        "write",
        "preview-update",
        "missing.accdb",
        "out.accdb",
        "--set",
        "Items",
        "Score",
        "oops",
        "int32",
        "10",
        "11",
    ]);
    assert!(malformed.run(None).unwrap_err().contains("ROW must"));
    let mixed = command(&[
        "tablecraft",
        "write",
        "preview-update",
        "missing.accdb",
        "out.accdb",
        "--variable",
        "--set",
        "Contacts",
        "Name",
        "0",
        "text",
        "Alice",
        "Bob",
        "--set",
        "Other",
        "Name",
        "0",
        "text",
        "Alice",
        "Bob",
    ]);
    assert!(
        mixed
            .run(None)
            .unwrap_err()
            .contains("one table and column")
    );
}

#[test]
fn quoted_tsv_preserves_null_empty_text_binary_and_companion_order() {
    let encoded = b"int32\t7\ttext\t\"Alice\tB\n\"\"C\"\"\"\tnull\t\tbinary\t00ff\nint32\t8\ttext\t\tnull\t\tbinary\t\n";
    let rows = input::inserts(encoded, 1024, RowInsert::new, &mut budget(), &|| false).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].values(),
        &[
            Value::Int32(7),
            Value::Text("Alice\tB\n\"C\"".to_owned()),
            Value::Null,
            Value::Binary(vec![0, 255])
        ]
    );
    assert_eq!(
        rows[1].values(),
        &[
            Value::Int32(8),
            Value::Text(String::new()),
            Value::Null,
            Value::Binary(Vec::new())
        ]
    );
    for invalid in [
        b"null\tnull\n".as_slice(),
        b"int32\tbad\n",
        b"text\n",
        b"text\tx\ntext\tx\tint32\t1\n",
        b"text\t\"unterminated",
        b"text\tbare\"quote\n",
        b"text\t\"closed\"junk\n",
        b"text\tx\r",
    ] {
        assert!(input::inserts(invalid, 1024, RowInsert::new, &mut budget(), &|| false).is_err());
    }
    assert!(input::inserts(encoded, 1, RowInsert::new, &mut budget(), &|| false).is_err());
    assert!(
        input::inserts(
            encoded,
            1024,
            RowInsert::new,
            &mut ResourceBudget::new(1, 1, 1),
            &|| false
        )
        .is_err()
    );
    assert!(matches!(
        input::inserts(encoded, 1024, RowInsert::new, &mut budget(), &|| true),
        Err(Error::Native(tablecraft_ace::InspectError::Cancelled))
    ));
}

#[test]
fn typed_update_escapes_distinguish_null_from_literal_null_without_cloning() {
    assert_eq!(
        values::update("text", "null", &mut budget()).unwrap(),
        Value::Null
    );
    assert_eq!(
        values::update("text", "value:null", &mut budget()).unwrap(),
        Value::Text("null".to_owned())
    );
    assert_eq!(
        values::update("text", "value:value:null", &mut budget()).unwrap(),
        Value::Text("value:null".to_owned())
    );
    assert!(values::update("misspelled", "null", &mut budget()).is_err());
    assert_eq!(
        values::value("null", "", false, &mut budget()).unwrap(),
        Value::Null
    );
    assert_eq!(
        values::value("text", "null", false, &mut budget()).unwrap(),
        Value::Text("null".to_owned())
    );
    assert!(
        values::value(
            "extended-date",
            "2024-02-30T00:00:00.0000000",
            false,
            &mut budget()
        )
        .is_err()
    );
    assert!(
        values::value(
            "extended-date",
            "2024-02-29T00:00:00.00000001",
            false,
            &mut budget()
        )
        .is_err()
    );
    let cells = [
        "Items", "Score", "0", "int32", "-2", "-1", "Items", "Score", "1", "int32", "null", "0",
    ]
    .map(str::to_owned);
    let groups = updates::parse(&cells, 1024, FixedScalarUpdate::new, &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].rows.len(), 2);
    assert_eq!(groups[0].rows[0].expected(), &Value::Int32(-2));
    assert_eq!(groups[0].rows[1].expected(), &Value::Null);
}

#[test]
fn source_counter_increment_policy_is_explicit_and_conflicts_before_input_open() {
    let args = [
        "tablecraft",
        "write",
        "preview-insert",
        "missing.accdb",
        "out.accdb",
        "Items",
        "missing.tsv",
        "--incrementing",
    ];
    match command(&args) {
        Command::Insert(insert) => {
            assert!(insert.incrementing);
            assert!(insert.incrementing_last.is_none());
        }
        _ => panic!("expected INSERT command"),
    }
    let mut conflicting = args.to_vec();
    conflicting.extend(["--incrementing-last", "0"]);
    assert!(Cli::try_parse_from(conflicting).is_err());
    let ordinary = &args[..args.len() - 1];
    match command(ordinary) {
        Command::Insert(insert) => {
            assert!(!insert.incrementing);
            assert!(insert.incrementing_last.is_none());
        }
        _ => panic!("expected INSERT command"),
    }
}
