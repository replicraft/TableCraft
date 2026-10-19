mod editing;
mod eval;
use crate::output;
mod args;
mod parameters;
mod retrieval;
mod saved;
pub(crate) use args::Command;
use std::fmt;
use tablecraft_expressions::EvalError;

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
