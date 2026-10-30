//! Rejected statement shapes stop before opening a database or output path.
use super::{Arguments, CreateIndexError, Error, run};
use crate::{Cli, commands};
use clap::Parser;
#[cfg(unix)]
use std::{cell::Cell, io::ErrorKind};
use std::{fs, path::PathBuf};
#[cfg(unix)]
use tablecraft_access::AccessInputError;
#[cfg(unix)]
use tablecraft_ace::InspectError;
#[cfg(unix)]
use tablecraft_testkit::access::secondary_index as model;

#[test]
fn durable_flag_is_limited_to_the_two_explicit_publication_consumers() {
    for (command, sql) in [
        ("update", "UPDATE Items SET Score=27 WHERE Id=2"),
        (
            "preview-create-index",
            "CREATE INDEX LookupScore ON Items (Score)",
        ),
    ] {
        assert!(
            Cli::try_parse_from([
                "tablecraft",
                "query",
                command,
                sql,
                "--database",
                "unopened.mdb",
                "--output",
                "unpublished.mdb",
                "--durable-output",
            ])
            .is_ok()
        );
    }
    assert!(
        Cli::try_parse_from([
            "tablecraft",
            "query",
            "preview-update",
            "UPDATE Items SET Score=27 WHERE Id=2",
            "--database",
            "unopened.accdb",
            "--output",
            "unpublished.accdb",
            "--durable-output",
        ])
        .is_err()
    );
}

#[test]
fn required_parent_capture_follows_closed_shape_and_precedes_source_open() {
    let directory = tempfile::tempdir().unwrap();
    let mut invalid = arguments("CREATE UNIQUE INDEX SecretIndex ON Items (Score)");
    invalid.database = directory.path().join("missing-source.accdb");
    invalid.output = directory.path().join("missing-parent/copy.accdb");
    invalid.durable_output = true;
    assert!(matches!(
        run(invalid, None),
        Err(Error::Definition(CreateIndexError::Unsupported))
    ));

    let mut valid = arguments("CREATE INDEX LookupScore ON Items (Score)");
    valid.database = directory.path().join("missing-source.accdb");
    valid.output = directory.path().join("missing-parent/copy.accdb");
    valid.durable_output = true;
    assert!(matches!(run(valid, None), Err(Error::Publish(_))));
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn index_source_release_failure_blocks_both_publication_modes() {
    let case = model::source(6, model::Population::Distinct, false);
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.accdb");
    fs::write(&input, &case.before).unwrap();
    for durable in [false, true] {
        let mut args = arguments("CREATE INDEX LookupScore ON Items (Score)");
        args.database = input.clone();
        args.output = directory.path().join("unpublished.accdb");
        args.durable_output = durable;
        let calls = Cell::new(0);
        let result = super::run_with_release(args, |source| {
            calls.set(calls.get() + 1);
            source.close()?;
            Err(AccessInputError::Native(InspectError::Io(
                ErrorKind::PermissionDenied,
            )))
        });
        assert!(matches!(result, Err(Error::SourceRelease(_))));
        assert_eq!(calls.get(), 1);
        assert!(!directory.path().join("unpublished.accdb").exists());
        assert_eq!(fs::read(&input).unwrap(), case.before);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

fn arguments(sql: &str) -> Arguments {
    Arguments {
        sql: sql.to_owned(),
        database: PathBuf::from("unopened-index-source.accdb"),
        output: PathBuf::from("unpublished-index-copy.accdb"),
        durable_output: false,
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
