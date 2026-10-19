use clap::{CommandFactory, Parser, Subcommand};
use std::io::{self, Write};
use std::process::ExitCode;

const INVALID_INPUT_STATUS: u8 = 2;

mod diagnostics;
mod inspect;
mod native;
mod output;
mod query;

#[derive(Parser)]
#[command(
    name = "tablecraft",
    version,
    about = "TableCraft inspection and query tools",
    after_help = "Query parse prints syntax; query eval calculates source-free SELECT expressions; query run executes bounded retrieval. Query update creates a fresh offline Jet 4 copy for qualified Int32 edits. Inspect reports metadata or explicit checks without executing content. In-place CLI editing and GUI hosts are not implemented."
)]
struct Cli {
    /// Enable operation diagnostics on stderr; repeat for I/O detail.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
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
    /// Parse/evaluate queries or create qualified offline UPDATE copies.
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
                Command::Query { command } => output(query::run(command)),
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
