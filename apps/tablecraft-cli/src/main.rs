use clap::{CommandFactory, Parser, Subcommand};
use std::io::{self, Write};
use std::process::ExitCode;

const INVALID_INPUT_STATUS: u8 = 2;

mod browse;
mod diagnostics;
mod inspect;
mod native;
mod output;
mod presentation;
mod query;

#[derive(Parser)]
#[command(
    name = "tablecraft",
    version,
    about = "TableCraft inspection and query tools",
    after_help = "Query parse prints syntax; query saved reconstructs a native saved SELECT without executing it; query eval calculates source-free SELECT expressions; query run executes bounded retrieval. Query update creates a fresh offline Jet 4 copy for qualified Int32 edits. Inspect reports metadata or explicit checks without executing content. In-place CLI editing and GUI hosts are not implemented."
)]
struct Cli {
    /// Output style for tables, rows and query run/eval; other reports keep their format.
    #[arg(long, global = true, value_enum, default_value = "auto")]
    format: presentation::Format,
    /// Enable operation diagnostics on stderr; repeat for I/O detail.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// List all table names in a stable/offline MDB/ACCDB, including system/linked tables.
    Tables {
        /// Explicit MDB/ACCDB file path; no linked sources or active content are opened.
        path: std::path::PathBuf,
        /// Show declared local/linked storage without opening tables or external sources.
        #[arg(long)]
        details: bool,
    },
    /// Display a page of local scalar table contents without writing SQL.
    #[command(
        after_help = "Reads a bounded page of local scalar records. Skipped and later row payloads are not decoded or checked. Uses physical row order, which can change between file copies; use query run with ORDER BY for sorted results. Use --stored-calculations to include qualified stored results marked [cached]; these may be stale and are never recalculated. Linked tables and unsupported field types fail explicitly. Use a stable/offline copy; shared-file coordination is not implemented."
    )]
    Rows {
        /// Explicit MDB/ACCDB file path.
        path: std::path::PathBuf,
        /// Exact table name (quote names containing spaces in your shell).
        table: String,
        /// Maximum displayed rows; native read/resource ceilings still apply.
        #[arg(long, default_value_t = browse::DEFAULT_PAGE_ROWS, value_parser = clap::value_parser!(u32).range(1..=browse::MAX_PAGE_ROWS as i64))]
        limit: u32,
        /// Skip this many live roots without decoding their field values.
        #[arg(long, default_value_t = 0)]
        offset: u32,
        /// Include qualified stored calculations, marked as cached; may be stale, never recalculated.
        #[arg(long)]
        stored_calculations: bool,
    },
    /// Inspect metadata or check tables on a stable/offline copy; executes no content.
    Inspect {
        /// Explicit MDB/ACCDB file path.
        path: std::path::PathBuf,
        /// Decode bounded object metadata; never executes or extracts object bodies.
        #[arg(long)]
        catalog: bool,
        /// Inspect one saved query's inert native fragments without executing SQL.
        #[arg(
            long,
            value_name = "NAME",
            conflicts_with_all = ["catalog", "relationships", "table", "indexes", "properties",
                "features", "complex", "stored_calculation", "complex_links", "check", "constraints"]
        )]
        query: Option<String>,
        /// Inspect inert relationship declarations, including composite mappings.
        #[arg(long, conflicts_with_all = ["catalog", "table", "indexes", "properties", "check", "constraints"])]
        relationships: bool,
        /// Select a local table for schema inspection or explicit checking.
        #[arg(long, value_name = "NAME", conflicts_with = "catalog")]
        table: Option<String>,
        /// Inspect native index metadata for the selected table.
        #[arg(long, requires = "table")]
        indexes: bool,
        /// Inspect inert native table/column/index properties without evaluating them.
        #[arg(long, requires = "table", conflicts_with_all = ["indexes", "check"])]
        properties: bool,
        /// Inspect inert calculated, complex and presentation field declarations.
        #[arg(long, requires = "table", conflicts_with_all = ["indexes", "properties", "check", "constraints", "relationships"])]
        features: bool,
        /// Resolve native complex fields to their type and child-table schemas.
        #[arg(long, requires = "table", conflicts_with_all = ["indexes", "properties", "features", "check", "constraints", "relationships"])]
        complex: bool,
        /// Inspect qualified stored calculation values without recalculating them.
        #[arg(long, value_name = "COLUMN", requires = "table", conflicts_with_all = ["indexes", "properties", "features", "complex", "check", "constraints", "relationships"])]
        stored_calculation: Option<String>,
        /// Inspect scoped complex parent keys and raw child memberships.
        #[arg(long, value_name="COLUMN", requires="table", conflicts_with_all=["indexes","properties","features","complex","stored_calculation","check","constraints","relationships"])]
        complex_links: Option<String>,
        /// Read all scalar rows and qualified indexes; report unchecked profiles.
        #[arg(long, requires = "table", conflicts_with = "indexes")]
        check: bool,
        /// Explicitly audit field declarations and qualified foreign-key records.
        #[arg(long, requires = "table", conflicts_with_all = ["indexes", "properties", "check"])]
        constraints: bool,
    },
    /// Inspect saved SELECTs, parse/evaluate queries or create qualified UPDATE copies.
    Query {
        #[command(subcommand)]
        command: query::Command,
    },
}
fn main() -> ExitCode {
    let (result, status) = match Cli::try_parse() {
        Ok(Cli { command: None, .. }) => (Cli::command().print_help(), ExitCode::SUCCESS),
        Ok(Cli {
            command: Some(command),
            verbose,
            format,
        }) => {
            if diagnostics::init(verbose).is_err() {
                let _ = writeln!(
                    io::stderr().lock(),
                    "error: could not initialize diagnostics"
                );
                return ExitCode::FAILURE;
            }
            let _span = tracing::debug_span!("cli.command").entered();
            match command {
                Command::Tables { path, details } => {
                    table_output(browse::tables(&path, details), format)
                }
                Command::Rows {
                    path,
                    table,
                    limit,
                    offset,
                    stored_calculations,
                } => table_output(
                    browse::rows(&path, &table, limit, offset, stored_calculations),
                    format,
                ),
                Command::Query { command } => {
                    if matches!(
                        &command,
                        query::Command::Run { .. } | query::Command::Eval { .. }
                    ) {
                        table_output(query::run(command), format)
                    } else {
                        output(query::run(command))
                    }
                }
                Command::Inspect {
                    path,
                    catalog,
                    query,
                    relationships,
                    table,
                    indexes,
                    properties,
                    features,
                    complex,
                    stored_calculation,
                    complex_links,
                    check,
                    constraints,
                } => {
                    let mode = if let Some(name) = query.as_deref() {
                        inspect::Mode::SavedQuery(name)
                    } else if relationships {
                        inspect::Mode::Relationships
                    } else if let Some(name) = table.as_deref() {
                        if let Some(column) = complex_links.as_deref() {
                            inspect::Mode::ComplexLinks(name, column)
                        } else if let Some(column) = stored_calculation.as_deref() {
                            inspect::Mode::StoredCalculation(name, column)
                        } else if complex {
                            inspect::Mode::Complex(name)
                        } else if features {
                            inspect::Mode::Features(name)
                        } else if check {
                            inspect::Mode::Check(name)
                        } else if constraints {
                            inspect::Mode::Constraints(name)
                        } else if properties {
                            inspect::Mode::Properties(name)
                        } else if indexes {
                            inspect::Mode::Indexes(name)
                        } else {
                            inspect::Mode::Table(name)
                        }
                    } else if catalog {
                        inspect::Mode::Catalog
                    } else {
                        inspect::Mode::Header
                    };
                    output(inspect::run(&path, mode))
                }
            }
        }
        Err(error) => (error.print(), ExitCode::from(error.exit_code() as u8)),
    };
    match result {
        Ok(()) => status,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "error: could not write output: {error}"
            );
            ExitCode::FAILURE
        }
    }
}

fn output<E: std::fmt::Display>(result: Result<String, E>) -> (io::Result<()>, ExitCode) {
    match result {
        Ok(text) => (
            io::stdout().lock().write_all(text.as_bytes()),
            ExitCode::SUCCESS,
        ),
        Err(error) => {
            tracing::debug!(success = false, "operation failed");
            (
                writeln!(io::stderr().lock(), "error: {error}"),
                ExitCode::from(INVALID_INPUT_STATUS),
            )
        }
    }
}

fn table_output<E: std::fmt::Display>(
    result: Result<String, E>,
    format: presentation::Format,
) -> (io::Result<()>, ExitCode) {
    output(
        result
            .map_err(|error| error.to_string())
            .and_then(|text| format.render(text).map_err(|error| error.to_string())),
    )
}
