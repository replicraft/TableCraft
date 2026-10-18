mod eval;
mod output;
use clap::Subcommand;
use std::fmt;
use tablecraft_expressions::EvalError;

#[derive(Subcommand)]
pub(super) enum Command {
    /// Parse one Access query and print normalized SQL; does not execute it.
    Parse {
        /// SQL text, passed as one shell-quoted argument.
        #[arg(allow_hyphen_values = true)]
        sql: String,
    },
    /// Evaluate a source-free SELECT using the supported scalar expression rules.
    #[command(
        after_help = "Accepts one SELECT of scalar expressions with optional aliases. Database sources, query clauses, parameters and action statements are not supported. Text comparison requires a collation and is not available here."
    )]
    Eval {
        /// SELECT text, passed as one shell-quoted argument.
        #[arg(allow_hyphen_values = true)]
        sql: String,
    },
}
pub(super) enum Error {
    Parse(tablecraft_sql::ParseError),
    Unsupported,
    Evaluation { column: usize, error: EvalError },
    Output(output::OutputError),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(f),
            Self::Unsupported => f.write_str("query eval requires a source-free SELECT with scalar projections and no query clauses or parameters"),
            Self::Evaluation { column, error } => write!(f, "column {column}: {error}"),
            Self::Output(error) => error.fmt(f),
        }
    }
}
pub(super) fn run(command: Command) -> Result<String, Error> {
    let sql = match &command {
        Command::Parse { sql } | Command::Eval { sql } => sql,
    };
    let parsed = tablecraft_sql::parse_query(sql, tablecraft_sql::ParseLimits::default())
        .map_err(Error::Parse)?;
    match command {
        Command::Parse { .. } => Ok(format!("{}\n", parsed.query())),
        Command::Eval { .. } => eval::run(parsed.query()),
    }
}
