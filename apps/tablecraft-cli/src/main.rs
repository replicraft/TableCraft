use clap::{CommandFactory, Parser, Subcommand};
use std::io::{self, Write};
use std::process::ExitCode;
mod inspect;
mod output;
mod query;

#[derive(Parser)]
#[command(
    name = "tablecraft",
    version,
    about = "TableCraft inspection and query tools",
    after_help = "Query parse prints syntax; query eval calculates source-free SELECT expressions; query run executes bounded inline or explicit native-table retrieval. Inspect reads native headers and optionally catalogs without executing content. Database editing is not implemented yet."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// Read native metadata from a stable/offline copy; executes no content.
    Inspect {
        /// Explicit MDB/ACCDB file path.
        path: std::path::PathBuf,
        /// Decode bounded object metadata; never executes or extracts object bodies.
        #[arg(long)]
        catalog: bool,
        /// Inspect column metadata for one local table; does not read its rows.
        #[arg(long, value_name = "NAME", conflicts_with = "catalog")]
        table: Option<String>,
    },
    /// Parse, evaluate scalar expressions or execute inline retrieval queries.
    Query {
        #[command(subcommand)]
        command: query::Command,
    },
}
fn main() -> ExitCode {
    let (result, status) = match Cli::try_parse() {
        Ok(Cli { command: None }) => (Cli::command().print_help(), ExitCode::SUCCESS),
        Ok(Cli {
            command: Some(command),
        }) => match command {
            Command::Query { command } => output(query::run(command)),
            Command::Inspect {
                path,
                catalog,
                table,
            } => output(inspect::run(&path, catalog, table.as_deref())),
        },
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
        Err(error) => (
            writeln!(io::stderr().lock(), "error: {error}"),
            ExitCode::from(2),
        ),
    }
}
