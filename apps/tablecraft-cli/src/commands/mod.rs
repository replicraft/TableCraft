//! CLI command arguments, dispatch and feature-specific handlers.
mod execution;
pub(crate) use execution::ExecutionError;
mod encryption;
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
    /// Display a page or stream local scalar table contents without writing SQL.
    #[command(
        after_help = "Reads a bounded page of local scalar records. Skipped and later row payloads are not decoded or checked. Uses physical row order, which can change between file copies; use query run with ORDER BY for sorted results. Use --stored-calculations to include qualified stored results marked [cached]; these may be stale and are never recalculated. Use --multivalues for qualified stored lists, or --collections for lists plus attachment metadata and stored Memo history. Complete parent-key and child snapshots retain their bounded read limits; attachments are never decoded implicitly and history remains in stored order. Linked tables and unsupported field types fail explicitly. Use --stream for progressive scalar TSV output; --limit becomes the batch size, and earlier output remains if a later read fails. Streaming accepts explicit --column selections and cumulative --native-bytes/--native-rows/--native-steps limits; it conflicts with offset, all-tables and stored-collection modes. Use a stable/offline copy; shared-file coordination is not implemented."
    )]
    Rows(rows::Args),
    /// Inspect metadata or check tables on a stable/offline copy; executes no content.
    Inspect(inspect::Args),
    /// Manage native database encryption.
    Encryption {
        #[command(subcommand)]
        command: encryption::Command,
    },
    /// Inspect saved SELECTs, parse/evaluate queries or create qualified UPDATE copies.
    Query {
        #[command(subcommand)]
        command: query::Command,
    },
}

impl Command {
    pub(crate) fn run(self, format: Format, password: Option<&str>) -> Result<String, String> {
        match self {
            Self::Encryption { command } => {
                command.run(format, password).map_err(|e| e.to_string())
            }
            Self::Tables(args) => render(
                tables::run(
                    &args.path,
                    args.details,
                    args.show_system,
                    args.count,
                    password,
                ),
                format,
            ),
            Self::Rows(args) => args
                .run(format, password)
                .map_err(|error| error.to_string()),
            Self::Inspect(args) => args
                .run(format, password)
                .map_err(|error| error.to_string()),
            Self::Query { command } => {
                if matches!(
                    &command,
                    query::Command::Run { .. } | query::Command::Eval { .. }
                ) {
                    let max_bytes = match &command {
                        query::Command::Run { large_values, .. } => large_values
                            .output_bytes
                            .map_or(crate::output::MAX_OUTPUT_BYTES, |bytes| bytes as usize),
                        _ => crate::output::MAX_OUTPUT_BYTES,
                    };
                    query::run(command, password)
                        .map_err(|error| error.to_string())
                        .and_then(|text| {
                            format
                                .render_with_limit(text, max_bytes)
                                .map_err(|error| error.to_string())
                        })
                } else {
                    query::run(command, password).map_err(|error| error.to_string())
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

mod template;
