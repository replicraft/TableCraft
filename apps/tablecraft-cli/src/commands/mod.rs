//! CLI command arguments, dispatch and feature-specific handlers.
mod error;
mod inspect;
mod query;
mod rows;
mod tables;

use crate::presentation::Format;
use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum Command {
    /// List user table names in a stable/offline MDB/ACCDB, including linked tables.
    Tables(tables::Args),
    /// Display a page of local scalar table contents without writing SQL.
    #[command(
        after_help = "Reads a bounded page of local scalar records. Skipped and later row payloads are not decoded or checked. Uses physical row order, which can change between file copies; use query run with ORDER BY for sorted results. Use --stored-calculations to include qualified stored results marked [cached]; these may be stale and are never recalculated. Use --multivalues for qualified stored lists; complete parent-key and child snapshots retain their bounded read limits. Linked tables and unsupported field types fail explicitly. Use a stable/offline copy; shared-file coordination is not implemented."
    )]
    Rows(rows::Args),
    /// Inspect metadata or check tables on a stable/offline copy; executes no content.
    Inspect(inspect::Args),
    /// Inspect saved SELECTs, parse/evaluate queries or create qualified UPDATE copies.
    Query {
        #[command(subcommand)]
        command: query::Command,
    },
}

impl Command {
    pub(crate) fn run(self, format: Format) -> Result<String, String> {
        match self {
            Self::Tables(args) => render(
                tables::run(&args.path, args.details, args.show_system),
                format,
            ),
            Self::Rows(args) => render(
                rows::run(
                    &args.path,
                    &args.table,
                    args.limit,
                    args.offset,
                    args.stored_calculations,
                    args.multivalues,
                ),
                format,
            ),
            Self::Inspect(args) => args.run(format).map_err(|error| error.to_string()),
            Self::Query { command } => {
                if matches!(
                    &command,
                    query::Command::Run { .. } | query::Command::Eval { .. }
                ) {
                    render(query::run(command), format)
                } else {
                    query::run(command).map_err(|error| error.to_string())
                }
            }
        }
    }
}

fn render<E: std::fmt::Display>(
    result: Result<String, E>,
    format: Format,
) -> Result<String, String> {
    result
        .map_err(|error| error.to_string())
        .and_then(|text| format.render(text).map_err(|error| error.to_string()))
}
