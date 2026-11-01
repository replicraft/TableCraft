//! Real clap routing and shared preflight/report consumers without file opens.
use super::super::{DeleteArguments, Error, delete, report::Reports};
use clap::Parser;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::RecoveryOutcome;

fn arguments(sql: &str) -> DeleteArguments {
    let parsed = crate::Cli::try_parse_from([
        "tablecraft",
        "query",
        "journal-delete",
        sql,
        "--database",
        "never-opened.accdb",
        "--journal",
        "never-created-journal",
    ])
    .unwrap_or_else(|error| panic!("argument parsing failed: {error}"));
    match parsed.command.unwrap() {
        crate::commands::Command::Query {
            command: crate::commands::query::Command::JournalDelete(arguments),
        } => *arguments,
        _ => panic!("expected journal-delete arguments"),
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn deletion_preflight_accepts_local_selection_and_refuses_other_actions_before_open() {
    let (_, collation) = delete::prepare(
        &arguments("DELETE FROM Sentinel WHERE ID=23"),
        &mut budget(),
    )
    .unwrap();
    assert!(collation.is_none());
    for sql in [
        "DELETE FROM [remote].[Sentinel] WHERE ID=23",
        "PARAMETERS p LONG; DELETE FROM Sentinel WHERE ID=p",
        "UPDATE Sentinel SET ID=24",
        "INSERT INTO Sentinel (ID) VALUES (24)",
        "SELECT 1",
    ] {
        assert!(matches!(
            delete::prepare(&arguments(sql), &mut budget()),
            Err(Error::Shape(_))
        ));
    }
    assert!(matches!(
        delete::prepare(&arguments("not SQL"), &mut budget()),
        Err(Error::Parse(_))
    ));
}

#[test]
fn deletion_password_refusal_precedes_even_malformed_sql_or_target_open() {
    for sql in ["DELETE FROM Sentinel WHERE ID=23", "not SQL"] {
        let parsed = crate::Cli::try_parse_from([
            "tablecraft",
            "query",
            "journal-delete",
            sql,
            "--database",
            "never-opened.accdb",
            "--journal",
            "never-created-journal",
        ])
        .unwrap();
        assert!(matches!(
            parsed.command.unwrap().execute(
                crate::presentation::Format::Auto,
                Some("explicit-test-password"),
                &mut Vec::new(),
            ),
            Err(crate::commands::ExecutionError::Journal(Error::Credentials))
        ));
    }
}

#[test]
fn deletion_report_uses_removed_root_counts_and_known_output_failure() {
    for count in [0, 1] {
        let mut output = Vec::new();
        Reports::delete(count, count, &mut budget())
            .unwrap()
            .publish(RecoveryOutcome::Committed, &mut output)
            .unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!(
                "Transaction\tCommitted\nMatched rows\t{count}\nDeleted rows\t{count}\nJournal namespace\tRetired and synchronized\n"
            )
        );
    }
    let mut writer = super::FailingWriter {
        fail_write: true,
        kind: std::io::ErrorKind::BrokenPipe,
        bytes: Vec::new(),
    };
    assert!(matches!(
        Reports::delete(1, 1, &mut budget())
            .unwrap()
            .publish(RecoveryOutcome::Committed, &mut writer),
        Err(Error::Output {
            outcome: RecoveryOutcome::Committed,
            ..
        })
    ));
}

#[test]
fn deletion_help_keeps_the_normal_action_boundary_explicit() {
    let help = crate::Cli::try_parse_from(["tablecraft", "query", "journal-delete", "--help"])
        .err()
        .expect("help is a display request")
        .to_string();
    let help = help.split_whitespace().collect::<Vec<_>>().join(" ");
    for expected in [
        "single-column Int32 root leaves and retained trees are supported",
        "other indexes, required programs and relationships refuse",
        "including empty operations",
        "no reclamation, cascade or compaction",
        "recover-update and finalize-update",
        "does not enable general ACE saves",
    ] {
        assert!(help.contains(expected), "missing help boundary: {expected}");
    }
}
