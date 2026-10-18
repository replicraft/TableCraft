use std::io::{self, Write};
use std::process::ExitCode;

use clap::{CommandFactory, Parser};

#[derive(Parser)]
#[command(
    name = "tablecraft",
    version,
    about = "TableCraft command-line foundation",
    after_help = "Database inspection and editing are not implemented yet."
)]
struct Cli;

fn main() -> ExitCode {
    let (result, status) = match Cli::try_parse() {
        Ok(_) => (Cli::command().print_help(), ExitCode::SUCCESS),
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
