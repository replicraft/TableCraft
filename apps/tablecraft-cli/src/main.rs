use clap::{CommandFactory, Parser, Subcommand};
use std::io::{self, Write};
use std::process::ExitCode;
mod query;

#[derive(Parser)]
#[command(
    name = "tablecraft",
    version,
    about = "TableCraft query tools",
    after_help = "Query parse prints syntax; query eval calculates source-free SELECT expressions; query run executes bounded inline retrieval. Database inspection and editing are not implemented yet."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
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
            command: Some(Command::Query { command }),
        }) => match query::run(command) {
            Ok(output) => (
                io::stdout().lock().write_all(output.as_bytes()),
                ExitCode::SUCCESS,
            ),
            Err(error) => (
                writeln!(io::stderr().lock(), "error: {error}"),
                ExitCode::from(2),
            ),
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
