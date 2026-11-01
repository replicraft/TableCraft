//! Real clap routing and shared preflight/report consumers without file opens.
use super::super::{Error, InsertArguments, insert, report::Reports};
use clap::Parser;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::RecoveryOutcome;

fn arguments(sql: &str) -> InsertArguments {
    let parsed = crate::Cli::try_parse_from([
        "tablecraft",
        "query",
        "journal-insert",
        sql,
        "--database",
        "never-opened.accdb",
        "--journal",
        "never-created-journal",
    ])
    .unwrap_or_else(|error| panic!("argument parsing failed: {error}"));
    match parsed.command.unwrap() {
        crate::commands::Command::Query {
            command: crate::commands::query::Command::JournalInsert(arguments),
        } => *arguments,
        _ => panic!("expected journal-insert arguments"),
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn insertion_preflight_accepts_complete_values_and_refuses_other_actions_before_open() {
    insert::prepare(
        &arguments("INSERT INTO Sentinel ([ID],Marker,Bytes) VALUES (24,'new',NULL)"),
        &mut budget(),
    )
    .unwrap();
    for sql in [
        "INSERT INTO Sentinel ([ID],Marker,Bytes) IN 'remote.accdb' VALUES (24,'new',NULL)",
        "PARAMETERS p LONG; INSERT INTO Sentinel ([ID],Marker,Bytes) VALUES (p,'new',NULL)",
        "UPDATE Sentinel SET ID=24",
        "INSERT INTO Sentinel ([ID], Marker, Bytes) SELECT [ID], Marker, Bytes FROM Sentinel",
        "SELECT 1",
    ] {
        assert!(matches!(
            insert::prepare(&arguments(sql), &mut budget()),
            Err(Error::Shape(_))
        ));
    }
    assert!(matches!(
        insert::prepare(&arguments("not SQL"), &mut budget()),
        Err(Error::Parse(_))
    ));
}

#[test]
fn insertion_password_refusal_precedes_even_malformed_sql_or_target_open() {
    for sql in [
        "INSERT INTO Sentinel ([ID],Marker,Bytes) VALUES (24,'new',NULL)",
        "not SQL",
    ] {
        let parsed = crate::Cli::try_parse_from([
            "tablecraft",
            "query",
            "journal-insert",
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
fn insertion_report_uses_inserted_counts_and_known_output_failure() {
    for count in [0, 1] {
        let mut output = Vec::new();
        Reports::insert(count, &mut budget())
            .unwrap()
            .publish(RecoveryOutcome::Committed, &mut output)
            .unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!(
                "Transaction\tCommitted\nInserted rows\t{count}\nJournal namespace\tRetired and synchronized\n"
            )
        );
    }
    let mut writer = super::FailingWriter {
        fail_write: true,
        kind: std::io::ErrorKind::BrokenPipe,
        bytes: Vec::new(),
    };
    assert!(matches!(
        Reports::insert(1, &mut budget())
            .unwrap()
            .publish(RecoveryOutcome::Committed, &mut writer),
        Err(Error::Output {
            outcome: RecoveryOutcome::Committed,
            ..
        })
    ));
}

#[test]
fn insertion_help_keeps_the_normal_action_boundary_explicit() {
    let help = crate::Cli::try_parse_from(["tablecraft", "query", "journal-insert", "--help"])
        .err()
        .expect("help is a display request")
        .to_string();
    let help = help.split_whitespace().collect::<Vec<_>>().join(" ");
    for expected in [
        "root-leaf indexes support nullable and descending keys",
        "other indexes and relationships refuse",
        "Supply every declared field exactly once",
        "Non-NULL Memo/OLE authoring is unsupported",
        "recover-update or finalize-update",
        "does not establish general ACE saves",
    ] {
        assert!(help.contains(expected), "missing help boundary: {expected}");
    }
}
