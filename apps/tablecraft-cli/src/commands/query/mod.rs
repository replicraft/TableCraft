mod binding_depth;
mod calendar;
mod character;
mod collation;
mod comparison_limits;
mod date_text;
mod editing;
mod eval;
mod execution_context;
mod execution_limits;
mod external;
mod guard_properties;
pub(in crate::commands) mod journal;
mod large_values;
mod links;
mod native_options;
mod native_target;
mod number;
pub(in crate::commands) mod preview_before_change;
use crate::output;
mod args;
mod parameters;
mod prepare_guards;
mod preview_calculation;
mod preview_create_index;
mod preview_delete;
mod preview_insert;
mod preview_update;
mod recomputed;
mod results;
mod retrieval;
mod run_args;
mod saved;
mod stream;
mod text_policy;
pub(crate) use args::Command;
use std::fmt;
use tablecraft_access::{CalculationError, ExternalSourceError, LinkSourceError};
use tablecraft_expressions::EvalError;
use tablecraft_formats::template::TemplateError;

pub(super) enum Error {
    Input(tablecraft_access::AccessInputError),
    Links(LinkSourceError),
    External(ExternalSourceError),
    Calculation(CalculationError),
    Recomputed(CalculationError),
    Template(TemplateError),
    Parse(tablecraft_sql::ParseError),
    Unsupported,
    JournalDispatch,
    BeforeChangeDispatch,
    Evaluation { column: usize, error: EvalError },
    Output(output::OutputError),
    Retrieval(tablecraft_query::QueryError),
    Parameter,
    Native(tablecraft_ace::InspectError),
    Sqlite(tablecraft_sqlite::SqliteError),
    NativeEdit(tablecraft_access::NativeEditError),
    Saved(tablecraft_access::SavedReadError),
    SavedRead(tablecraft_access::NativeSavedReadError),
    Update(tablecraft_query::UpdateError<tablecraft_access::NativeEditError>),
    Copy(crate::native::publication::Error),
    EngineeringDelete(preview_delete::Error),
    EngineeringInsert(preview_insert::Error),
    EngineeringUpdate(preview_update::Error),
    EngineeringIndex(preview_create_index::Error),
    GuardPreparation(prepare_guards::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => error.fmt(f),
            Self::Links(error) => error.fmt(f),
            Self::External(error) => error.fmt(f),
            Self::Calculation(error) => match error {
                CalculationError::Native(error) => write!(f, "calculation metadata: {error}"),
                CalculationError::Parse(error) => write!(f, "calculation syntax: {error}"),
                CalculationError::Query(error) => write!(f, "calculation preview: {error}"),
                error => error.fmt(f),
            },
            Self::Recomputed(error) => write!(f, "recomputed view: {error}"),
            Self::Template(error) => error.fmt(f),
            Self::Parse(error) => error.fmt(f),
            Self::Unsupported => f.write_str("query eval requires a source-free SELECT with scalar projections and no query clauses or parameters"),
            Self::Evaluation { column, error } => write!(f, "column {column}: {error}"),
            Self::Output(error) => error.fmt(f),
            Self::Retrieval(error) => error.fmt(f),
            Self::Native(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
            Self::NativeEdit(error) => error.fmt(f),
            Self::Saved(error) => error.fmt(f),
            Self::SavedRead(error) => error.fmt(f),
            Self::Update(error) => error.fmt(f),
            Self::Copy(error) => error.fmt(f),
            Self::EngineeringDelete(error) => error.fmt(f),
            Self::EngineeringInsert(error) => error.fmt(f),
            Self::EngineeringUpdate(error) => error.fmt(f),
            Self::EngineeringIndex(error) => error.fmt(f),
            Self::GuardPreparation(error) => error.fmt(f),
            Self::Parameter => f.write_str("invalid query run parameter"),
            Self::BeforeChangeDispatch => f.write_str("BeforeChange copies require publication-aware execution"),
            Self::JournalDispatch => f.write_str("journal commands require the explicit outcome-aware execution path"),
        }
    }
}
pub(super) fn run(command: Command, password: Option<&str>) -> Result<String, Error> {
    match command {
        Command::PreviewBeforeChange(_) => Err(Error::BeforeChangeDispatch),
        Command::JournalUpdate(_) | Command::RecoverUpdate(_) | Command::FinalizeUpdate(_) => {
            Err(Error::JournalDispatch)
        }
        Command::PrepareGuards(args) => {
            prepare_guards::run(*args, password).map_err(Error::GuardPreparation)
        }
        Command::PreviewInsert(args) => {
            preview_insert::run(*args, password).map_err(Error::EngineeringInsert)
        }
        Command::PreviewDelete(args) => {
            preview_delete::run(*args, password).map_err(Error::EngineeringDelete)
        }
        Command::PreviewCreateIndex(args) => {
            preview_create_index::run(*args, password).map_err(Error::EngineeringIndex)
        }
        Command::PreviewUpdate(args) => {
            preview_update::run(*args, password).map_err(Error::EngineeringUpdate)
        }
        Command::PreviewCalculation(args) => preview_calculation::run(*args, password),
        Command::Saved { name, database } => saved::run(&database, &name, password),
        Command::Parse { sql } => Ok(format!("{}\n", parse(&sql)?.query())),
        Command::Eval {
            sql,
            year_window,
            character_code_page,
            number_format,
            date_format,
            reference_year,
            first_day_of_week,
            first_week_of_year,
            case_locale,
            clock,
            random_seed,
        } => eval::run(
            parse(&sql)?.query(),
            eval::Controls {
                year_window,
                character_code_page,
                number_format,
                date_format,
                reference_year,
                first_day_of_week,
                first_week_of_year,
                case_locale,
                clock,
                random_seed,
            },
        ),
        Command::Run(arguments) => arguments.run_buffered(password),
        Command::Update {
            sql,
            database,
            output,
            collation,
            durable_output,
        } => editing::run(
            parse(&sql)?.query(),
            &database,
            &output,
            collation.map(Into::into),
            durable_output,
        ),
    }
}
fn parse(sql: &str) -> Result<tablecraft_sql::ParsedQuery, Error> {
    tablecraft_sql::parse_query(sql, tablecraft_sql::ParseLimits::default()).map_err(Error::Parse)
}
