mod editing;
mod eval;
use crate::output;
mod parameters;
mod retrieval;
mod saved;
use clap::Subcommand;
use std::fmt;
use tablecraft_expressions::EvalError;

#[derive(Subcommand)]
pub(super) enum Command {
    /// Reconstruct one native saved local SELECT; prints SQL without executing it.
    #[command(
        after_help = "Reads explicitly selected native metadata from a stable/offline copy. Supports the qualified local SELECT profile; unknown, conflicting and unsupported fragments fail. This does not bind functions, expand saved sources or execute SQL. Use inspect --query for raw fragments of other query kinds."
    )]
    Saved {
        /// Native query name, passed as one shell-quoted argument.
        #[arg(value_name = "NAME")]
        name: String,
        /// Explicit stable/offline MDB/ACCDB copy, opened read-only.
        #[arg(long, value_name = "PATH")]
        database: std::path::PathBuf,
    },
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
    /// Execute bounded retrieval over inline rows, MDB/ACCDB or SQLite tables.
    #[command(
        after_help = "Supports inline SELECT/UNION sources, filtering, joins, grouping and subqueries. Use --database with a stable/offline copy for supported local scalar tables; linked tables, saved queries and unsupported column types fail explicitly. Parameters use NAME=TYPE:VALUE (boolean, byte, short, long, bigint, text, date, guid or null). Text comparisons require an explicit --collation; ordinal is not an Access locale collation. LIKE also requires --like-mode ansi89 or ansi92; database wildcard settings are not inferred."
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
        /// Explicit LIKE wildcards; database ANSI settings are not inferred.
        #[arg(long, value_enum)]
        like_mode: Option<retrieval::Wildcards>,
        /// Read supported local tables from this explicit stable/offline MDB/ACCDB copy.
        #[arg(long, value_name = "PATH")]
        database: Option<std::path::PathBuf>,
        /// Read supported scalar tables from an existing SQLite file, read-only.
        #[arg(long, value_name = "PATH", conflicts_with_all = ["database", "index"])]
        sqlite: Option<std::path::PathBuf>,
        /// Require a supported native index for this table's snapshot.
        #[arg(long, value_names = ["TABLE", "INDEX"], num_args = 2, requires = "database", action = clap::ArgAction::Set)]
        index: Vec<String>,
    },
    /// Apply a bounded single-table UPDATE to a fresh offline Jet 4 copy.
    #[command(
        after_help = "Supports one existing non-indexed non-NULL fixed Int32 assignment with pure expressions and WHERE. Requires an offline Jet 4 source and a new output path; never modifies the input. ACE, INSERT/DELETE, joins, parameters and unavailable rules/content are rejected. A write failure after output creation may leave an incomplete copy. Text filters require an explicit collation; ordinal is not an Access locale collation."
    )]
    Update {
        /// UPDATE text, passed as one shell-quoted argument.
        #[arg(allow_hyphen_values = true)]
        sql: String,
        /// Existing stable/offline MDB input, opened read-only under an OS lock.
        #[arg(long, value_name = "PATH")]
        database: std::path::PathBuf,
        /// Fresh output copy; an existing path is never overwritten.
        #[arg(long, value_name = "PATH")]
        output: std::path::PathBuf,
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
    Native(tablecraft_ace::InspectError),
    Sqlite(tablecraft_sqlite::SqliteError),
    NativeEdit(tablecraft_access::NativeEditError),
    Saved(tablecraft_access::SavedSelectError),
    Update(tablecraft_query::UpdateError<tablecraft_access::NativeEditError>),
    Copy(editing::CopyError),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(f),
            Self::Unsupported => f.write_str("query eval requires a source-free SELECT with scalar projections and no query clauses or parameters"),
            Self::Evaluation { column, error } => write!(f, "column {column}: {error}"),
            Self::Output(error) => error.fmt(f),
            Self::Retrieval(error) => error.fmt(f),
            Self::Native(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
            Self::NativeEdit(error) => error.fmt(f),
            Self::Saved(error) => error.fmt(f),
            Self::Update(error) => error.fmt(f),
            Self::Copy(error) => error.fmt(f),
            Self::Parameter => f.write_str("invalid query run parameter"),
        }
    }
}
pub(super) fn run(command: Command) -> Result<String, Error> {
    match command {
        Command::Saved { name, database } => saved::run(&database, &name),
        Command::Parse { sql } => Ok(format!("{}\n", parse(&sql)?.query())),
        Command::Eval { sql } => eval::run(parse(&sql)?.query()),
        Command::Run {
            sql,
            parameters,
            collation,
            like_mode,
            database,
            sqlite,
            index,
        } => retrieval::run(
            parse(&sql)?.query(),
            &parameters,
            collation,
            like_mode,
            database.as_deref(),
            sqlite.as_deref(),
            &index,
        ),
        Command::Update {
            sql,
            database,
            output,
            collation,
        } => editing::run(parse(&sql)?.query(), &database, &output, collation),
    }
}
fn parse(sql: &str) -> Result<tablecraft_sql::ParsedQuery, Error> {
    tablecraft_sql::parse_query(sql, tablecraft_sql::ParseLimits::default()).map_err(Error::Parse)
}
