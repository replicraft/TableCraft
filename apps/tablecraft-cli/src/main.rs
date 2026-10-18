use clap::{CommandFactory, Parser, Subcommand};
use std::io::{self, Write};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "tablecraft",
    version,
    about = "TableCraft query syntax tools",
    after_help = "Query parsing does not execute SQL. Database inspection and editing are not implemented yet."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// Work with query syntax without connecting to a database.
    Query {
        #[command(subcommand)]
        command: QueryCommand,
    },
}
#[derive(Subcommand)]
enum QueryCommand {
    /// Parse one Access query and print normalized SQL; does not execute it.
    Parse {
        /// SQL text, passed as one shell-quoted argument.
        #[arg(allow_hyphen_values = true)]
        sql: String,
    },
}
fn main() -> ExitCode {
    let (result, status) = match Cli::try_parse() {
        Ok(Cli { command: None }) => (Cli::command().print_help(), ExitCode::SUCCESS),
        Ok(Cli {
            command:
                Some(Command::Query {
                    command: QueryCommand::Parse { sql },
                }),
        }) => match tablecraft_sql::parse_query(&sql, tablecraft_sql::ParseLimits::default()) {
            Ok(parsed) => (
                writeln!(io::stdout().lock(), "{}", parsed.query()),
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
