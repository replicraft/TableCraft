use clap::{CommandFactory, Parser};
use commands::{Command, ExecutionError};
use std::io::{self, Write};
use std::process::ExitCode;

const INVALID_INPUT_STATUS: u8 = 2;

mod commands;
mod diagnostics;
mod native;
mod output;
mod presentation;
mod terminal_wait;

#[derive(Parser)]
#[command(
    name = "tablecraft",
    version,
    about = "TableCraft inspection and query tools",
    after_help = concat!(
        "Query parse prints syntax; query saved reconstructs a native saved SELECT without executing it; ",
        "query eval calculates source-free SELECT expressions; query run executes bounded retrieval; ",
        "query preview-calculation explicitly recomputes a restricted stored formula without saving. ",
        "Query update creates a fresh offline Jet 4 copy for qualified Int32 edits. ",
        "Open receives a file for inert header inspection. ",
        "Inspect reports metadata or explicit checks without executing content. ",
        "In-place CLI editing and GUI hosts are not implemented."
    )
)]
struct Cli {
    /// Output style for open, tables, rows, inspect and query run/eval/preview-calculation.
    #[arg(long, global = true, value_enum, default_value = "auto")]
    format: presentation::Format,
    /// Enable operation diagnostics on stderr; repeat for I/O detail.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
    /// Database password; prompts with hidden input on a terminal when omitted.
    #[arg(short = 'p', long, global = true, value_name = "PASSWORD")]
    password: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

fn main() -> ExitCode {
    let mut wait_for_terminal = false;
    let (result, status) = match Cli::try_parse() {
        Ok(Cli { command: None, .. }) => (Cli::command().print_help(), ExitCode::SUCCESS),
        Ok(Cli {
            command: Some(command),
            verbose,
            format,
            password,
        }) => {
            wait_for_terminal = command.waits_for_terminal();
            if diagnostics::init(verbose).is_err() {
                let _ = writeln!(
                    io::stderr().lock(),
                    "error: could not initialize diagnostics"
                );
                return finish(ExitCode::FAILURE, wait_for_terminal);
            }
            let _span = tracing::debug_span!("cli.command").entered();
            let password = password.map(zeroize::Zeroizing::new);
            output(command.execute(
                format,
                password.as_ref().map(|p| p.as_str()),
                &mut io::stdout().lock(),
            ))
        }
        Err(error) => (error.print(), ExitCode::from(error.exit_code() as u8)),
    };
    let status = match result {
        Ok(()) => status,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "error: could not write output: {error}"
            );
            ExitCode::FAILURE
        }
    };

    finish(status, wait_for_terminal)
}

fn finish(status: ExitCode, wait_for_terminal: bool) -> ExitCode {
    if let Err(error) = terminal_wait::after_publication(wait_for_terminal) {
        let _ = writeln!(
            io::stderr().lock(),
            "error: could not wait for terminal input: {error}"
        );
        return ExitCode::FAILURE;
    }

    status
}

fn output(result: Result<(), ExecutionError>) -> (io::Result<()>, ExitCode) {
    match result {
        Ok(()) => (Ok(()), ExitCode::SUCCESS),
        Err(ExecutionError::Output(error)) => (Err(error), ExitCode::FAILURE),
        Err(ExecutionError::Operation(error)) => {
            tracing::debug!(success = false, "operation failed");
            (
                writeln!(io::stderr().lock(), "error: {error}"),
                ExitCode::from(INVALID_INPUT_STATUS),
            )
        }
    }
}
