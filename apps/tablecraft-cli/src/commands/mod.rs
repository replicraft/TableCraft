//! CLI command arguments, dispatch and feature-specific handlers.
mod execution;
pub(crate) use execution::ExecutionError;
mod encryption;
mod error;
mod inspect;
mod query;
mod rows;
mod tables;
mod write;

use crate::presentation::Format;
use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum Command {
    /// List user table names in a stable/offline MDB/ACCDB, including linked tables.
    Tables(tables::Args),
    /// Browse stored local table contents or stream ordinary scalar rows without SQL.
    #[command(
        after_help = concat!(
            "The table name is optional when exactly one user table exists; system tables do not count. ",
            "Otherwise supply a table name or use --all. Buffered browsing includes qualified stored ",
            "collections and calculation caches marked [cached]. Caches may be stale; formulas, lookup ",
            "queries and attachment decoders never run. Complete collection snapshots retain their ",
            "bounded read limits. Unsupported fields and linked tables fail explicitly. ",
            "Ordinary scalar pages leave skipped and later payloads unchecked. Physical row order can ",
            "change between copies; use query run with ORDER BY for sorted results.\n\n",
            "Use --stream for progressive ordinary scalar TSV output; --limit becomes the batch size, ",
            "and earlier output remains if a later read fails. Streaming accepts explicit --column ",
            "selections and cumulative --native-bytes/--native-rows/--native-steps limits; it conflicts ",
            "with offset, all-tables and stored-collection modes. Use a stable/offline copy; live Access ",
            "snapshot coordination is not implemented."
        )
    )]
    Rows(rows::Args),
    /// Inspect metadata or check tables on a stable/offline copy; executes no content.
    Inspect(inspect::Args),
    /// Manage native database encryption.
    Encryption {
        #[command(subcommand)]
        command: encryption::Command,
    },
    /// Create explicit experimental ACE engineering copies; never normal saves.
    #[command(
        after_help = "Use only stable/offline sources and disposable fresh outputs. Required data macros are neither qualified nor executed. These preview commands never overwrite sources, commit journals or enable normal ACE saves. Encrypted rewrites and deployment-package sources are refused."
    )]
    Write {
        #[command(subcommand)]
        command: write::Command,
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
                    args.native_bytes,
                ),
                format,
            ),
            Self::Rows(args) => args
                .run(format, password)
                .map_err(|error| error.to_string()),
            Self::Inspect(args) => args
                .run(format, password)
                .map_err(|error| error.to_string()),
            Self::Write { command } => command.run(password),
            Self::Query { command } => {
                if matches!(
                    &command,
                    query::Command::Run(_)
                        | query::Command::Eval { .. }
                        | query::Command::PreviewCalculation(_)
                ) {
                    let max_bytes = match &command {
                        query::Command::Run(arguments) => arguments
                            .large_values
                            .output_bytes
                            .map_or(crate::output::MAX_OUTPUT_BYTES, |bytes| bytes as usize),
                        query::Command::PreviewCalculation(arguments) => arguments
                            .large_values
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
