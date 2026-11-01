//! Child of journal::tests; no filesystem or source fixture is needed.
use super::super::Error;
use clap::Parser;

#[test]
fn password_refusal_precedes_even_malformed_batch_sql() {
    for sql in ["not a SQL statement", "UPDATE Items SET Flag=True,[Money]="] {
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
        .unwrap_or_else(|error| panic!("clap rejected SQL argument text: {error}"));
        let result = cli.command.unwrap().execute(
            crate::presentation::Format::Auto,
            Some("explicit-test-password"),
            &mut Vec::new(),
        );
        assert!(matches!(
            result,
            Err(crate::commands::ExecutionError::Journal(Error::Credentials))
        ));
    }
}
