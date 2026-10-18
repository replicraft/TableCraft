mod eval;
mod output;
mod parameters;
mod retrieval;
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
    /// Execute bounded retrieval over inline derived tables; opens no databases.
    #[command(
        after_help = "Supports inline SELECT/UNION sources, filtering, joins, grouping and subqueries. Named database tables and native files are unavailable. Parameters use NAME=TYPE:VALUE (boolean, byte, short, long, bigint, text or null). Text comparisons require an explicit --collation; ordinal is not an Access locale collation."
    )]
    Run {
        /// Retrieval SQL text, passed as one shell-quoted argument.
        #[arg(allow_hyphen_values = true)]
        sql: String,
        /// Declared parameter, for example Floor=bigint:4 or Label=text:hello.
        #[arg(long = "param", value_name = "NAME=TYPE:VALUE")]
        parameters: Vec<String>,
        /// Explicit text comparison policy.
        #[arg(long, value_enum)]
        collation: Option<retrieval::TextCollation>,
    },
}
pub(super) enum Error {
    Parse(tablecraft_sql::ParseError),
    Unsupported,
    Evaluation { column: usize, error: EvalError },
    Output(output::OutputError),
    Retrieval(tablecraft_query::QueryError),
    Parameter,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(f),
            Self::Unsupported => f.write_str("query eval requires a source-free SELECT with scalar projections and no query clauses or parameters"),
            Self::Evaluation { column, error } => write!(f, "column {column}: {error}"),
            Self::Output(error) => error.fmt(f),
            Self::Retrieval(error) => error.fmt(f),
            Self::Parameter => f.write_str("invalid query run parameter"),
        }
    }
}
pub(super) fn run(command: Command) -> Result<String, Error> {
    let sql = match &command {
        Command::Parse { sql } | Command::Eval { sql } | Command::Run { sql, .. } => sql,
    };
    let parsed = tablecraft_sql::parse_query(sql, tablecraft_sql::ParseLimits::default())
        .map_err(Error::Parse)?;
    match command {
        Command::Parse { .. } => Ok(format!("{}\n", parsed.query())),
        Command::Eval { .. } => eval::run(parsed.query()),
        Command::Run {
            parameters,
            collation,
            ..
        } => retrieval::run(parsed.query(), &parameters, collation),
    }
}
