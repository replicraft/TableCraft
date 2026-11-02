use super::super::{Error, UpdateArguments, preflight};
use clap::Parser;
use tablecraft_core::limits::ResourceBudget;

fn arguments(sql: &str) -> UpdateArguments {
    let cli = crate::Cli::try_parse_from([
        "tablecraft",
        "query",
        "journal-update",
        sql,
        "--database",
        "never-opened-target.accdb",
        "--journal",
        "never-created-journal",
    ])
    .unwrap_or_else(|error| panic!("argument parsing failed: {error}"));
    match cli.command.unwrap() {
        crate::commands::Command::Query {
            command: crate::commands::query::Command::JournalUpdate(arguments),
        } => *arguments,
        _ => panic!("expected journal-update arguments"),
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn journal_preflight_selects_exact_single_or_batch_service_without_opening() {
    for (sql, expected) in [
        (
            "UPDATE Items SET Score=12 WHERE Id=1",
            preflight::Route::Single,
        ),
        (
            "UPDATE Items SET Flag=True,Int64Value=CLngLng(42) WHERE Int32Value=0",
            preflight::Route::Batch,
        ),
    ] {
        let (_, collation, route) = preflight::prepare(&arguments(sql), &mut budget()).unwrap();
        assert_eq!(route, expected);
        assert!(collation.is_none());
    }
}

#[test]
fn journal_batch_preflight_preserves_external_parameter_and_action_refusals() {
    for sql in [
        "UPDATE [remote].[Items] SET Flag=True,Int64Value=CLngLng(42)",
        "PARAMETERS p LONG; UPDATE Items SET Flag=True,Int64Value=CLngLng(p)",
        "INSERT INTO Items (Id,Score) VALUES (1,12)",
        "DELETE FROM Items WHERE Id=1",
        "SELECT 1",
    ] {
        assert!(matches!(
            preflight::prepare(&arguments(sql), &mut budget()),
            Err(Error::Shape(_))
        ));
    }
}

#[test]
fn journal_batch_password_refusal_still_precedes_preflight_and_open() {
    let cli = crate::Cli::try_parse_from([
        "tablecraft",
        "query",
        "journal-update",
        "UPDATE Items SET Flag=True,Int64Value=CLngLng(42)",
        "--database",
        "never-opened-target.accdb",
        "--journal",
        "never-created-journal",
    ])
    .unwrap_or_else(|error| panic!("argument parsing failed: {error}"));
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
fn journal_help_describes_fixed_and_fitted_updates_without_obsolete_limits() {
    let error = crate::Cli::try_parse_from(["tablecraft", "query", "journal-update", "--help"])
        .err()
        .expect("help is a display request");
    let help = error
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    assert!(help.contains("Short Text/Binary batches"));
    assert!(help.contains("fitting NULL-key transitions"));
    assert!(help.contains("Int64/Currency index updates"));
    assert!(help.contains("retained NODE partitions without splitting"));
    assert!(help.contains("external single/chained payload reuse"));
    assert!(help.contains("external growth, clearing and allocation refuse"));
    assert!(help.contains("one represented EOF extension"));
    assert!(help.contains("AutoNumber fields remain read-only"));
    assert!(help.contains("All SET expressions read one original snapshot"));
    assert!(help.contains("does not enable general ACE saves"));
    assert!(!help.contains("Indexed NULL target transitions remain refused"));
    assert!(!help.contains("indirect maps remain refused"));
    assert!(!help.contains("at most 64 pages"));
    assert!(!help.contains("page allocation and file growth remain unsupported"));
}
