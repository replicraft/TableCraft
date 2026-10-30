//! Rejected statement shapes stop before opening a database or output path.
use super::{Arguments, CreateIndexError, Error, run};
use crate::{Cli, commands};
use clap::Parser;
use std::path::PathBuf;

fn arguments(sql: &str) -> Arguments {
    Arguments {
        sql: sql.to_owned(),
        database: PathBuf::from("unopened-index-source.accdb"),
        output: PathBuf::from("unpublished-index-copy.accdb"),
        work_bytes: super::WORK_BYTES,
        work_rows: super::WORK_ROWS,
        work_steps: super::WORK_STEPS,
    }
}

#[test]
fn create_index_command_is_explicit_and_retains_standard_capture_options() {
    let parsed = Cli::try_parse_from([
        "tablecraft",
        "query",
        "preview-create-index",
        "CREATE INDEX LookupScore ON Items (Score DESC)",
        "--database",
        "unopened-index-source.accdb",
        "--output",
        "unpublished-index-copy.accdb",
    ])
    .unwrap();
    let Some(commands::Command::Query { command }) = parsed.command else {
        panic!("query command");
    };
    assert!(matches!(
        command,
        super::super::Command::PreviewCreateIndex(_)
    ));

    assert!(
        Cli::try_parse_from([
            "tablecraft",
            "query",
            "preview-create-index",
            "CREATE INDEX LookupScore ON Items (Score)",
            "--database",
            "unopened-index-source.accdb",
            "--output",
            "unpublished-index-copy.accdb",
            "--image-bytes",
            "83886080",
        ])
        .is_err()
    );
}

#[test]
fn unsupported_definitions_refuse_before_database_open_without_identifier_echo() {
    for sql in [
        "SELECT 1",
        "CREATE TABLE SecretTable (Id LONG)",
        "DROP INDEX SecretIndex ON SecretTable",
        "CREATE UNIQUE INDEX SecretIndex ON SecretTable (SecretColumn)",
        "CREATE INDEX SecretIndex ON SecretTable (SecretColumn) WITH PRIMARY",
        "CREATE INDEX SecretIndex ON SecretTable (SecretColumn) WITH DISALLOW NULL",
        "CREATE INDEX SecretIndex ON SecretTable (SecretColumn) WITH IGNORE NULL",
        "CREATE INDEX SecretIndex ON SecretTable (SecretColumn, Other)",
    ] {
        let result = run(arguments(sql), None);
        assert!(matches!(
            &result,
            Err(Error::Definition(CreateIndexError::Unsupported))
        ));
        let message = result.err().unwrap().to_string();
        assert!(!message.contains("Secret"));
    }
}

#[test]
fn malformed_or_unrepresentable_syntax_is_not_rewritten_or_retried() {
    for sql in [
        "CREATE INDEX",
        "CREATE INDEX LookupScore ON Items ()",
        "CREATE INDEX LookupScore ON Items (Score + 1)",
        "CREATE INDEX [Bad]]Name] ON Items (Score)",
    ] {
        assert!(matches!(run(arguments(sql), None), Err(Error::Parse(_))));
    }
}
