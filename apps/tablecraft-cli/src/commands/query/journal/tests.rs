use super::{Error, report::Reports};
use std::io::{self, Write};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::RecoveryOutcome;

struct FailingWriter {
    fail_write: bool,
    kind: io::ErrorKind,
    bytes: Vec<u8>,
}

impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            return Err(self.kind.into());
        }

        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(self.kind.into())
    }
}

#[test]
fn completion_output_failure_preserves_each_known_outcome() {
    for outcome in [RecoveryOutcome::Committed, RecoveryOutcome::RolledBack] {
        for fail_write in [true, false] {
            for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::Other] {
                let mut budget = ResourceBudget::new(4096, 10, 4096);
                let reports = Reports::recovery(&mut budget).unwrap();
                let mut writer = FailingWriter {
                    fail_write,
                    kind,
                    bytes: Vec::new(),
                };
                let error = reports.publish(outcome, &mut writer).unwrap_err();
                assert!(matches!(
                    &error,
                    Error::Output { outcome: actual, error }
                        if *actual == outcome && error.kind() == kind
                ));
                assert!(!error.invalid_input());
                assert!(error.to_string().contains("do not retry the UPDATE"));
                assert_eq!(writer.bytes.is_empty(), fail_write);
            }
        }
    }
}

#[test]
fn complete_report_has_exact_retirement_and_row_fields() {
    let mut budget = ResourceBudget::new(4096, 10, 4096);
    let reports = Reports::update(3, 2, &mut budget).unwrap();
    let mut output = Vec::new();
    reports
        .publish(RecoveryOutcome::Committed, &mut output)
        .unwrap();
    assert_eq!(
        output,
        b"Transaction\tCommitted\nMatched rows\t3\nChanged rows\t2\nJournal namespace\tRetired and synchronized\n"
    );
}

#[test]
fn preopen_dispatch_rejects_passwords_and_unsupported_shapes() {
    use clap::Parser;

    for sql in [
        "UPDATE Items SET Score=12, Id=2",
        "UPDATE [remote].[Items] SET Score=12",
        "DELETE FROM Items WHERE Id=1",
    ] {
        let cli = crate::Cli::try_parse_from([
            "tablecraft",
            "query",
            "journal-update",
            sql,
            "--database",
            "missing-target",
            "--journal",
            "missing-parent/bundle",
        ])
        .unwrap_or_else(|error| panic!("clap rejected test SQL argument: {error}"));
        let result =
            cli.command
                .unwrap()
                .execute(crate::presentation::Format::Auto, None, &mut Vec::new());
        assert!(matches!(
            result,
            Err(crate::commands::ExecutionError::Journal(Error::Shape(_)))
        ));
    }

    let cli = crate::Cli::try_parse_from([
        "tablecraft",
        "query",
        "journal-update",
        "UPDATE Items SET Score=12 WHERE Id=1",
        "--database",
        "missing-target",
        "--journal",
        "missing-parent/bundle",
    ])
    .unwrap_or_else(|error| panic!("clap rejected test SQL argument: {error}"));
    assert!(matches!(
        cli.command.unwrap().execute(
            crate::presentation::Format::Auto,
            Some("unused"),
            &mut Vec::new(),
        ),
        Err(crate::commands::ExecutionError::Journal(Error::Credentials))
    ));
}

#[test]
fn journal_command_arguments_keep_recovery_free_of_sql_and_copy_flags() {
    use clap::Parser;

    assert!(
        crate::Cli::try_parse_from([
            "tablecraft",
            "query",
            "journal-update",
            "UPDATE Items SET Score=12 WHERE Id=1",
            "--database",
            "offline.mdb",
            "--journal",
            "edit-journal",
            "--work-bytes",
            "1024",
            "--work-rows",
            "10",
            "--work-steps",
            "1000",
        ])
        .is_ok()
    );

    for extra in ["UPDATE Items SET Score=12", "--output", "--durable-output"] {
        assert!(
            crate::Cli::try_parse_from([
                "tablecraft",
                "query",
                "recover-update",
                "--database",
                "offline.mdb",
                "--journal",
                "edit-journal",
                extra,
            ])
            .is_err()
        );
    }
}
