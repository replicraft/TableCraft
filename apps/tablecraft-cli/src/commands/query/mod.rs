mod calendar;
mod character;
mod clock;
mod collation;
mod comparison_limits;
mod date_text;
mod editing;
mod eval;
mod execution_limits;
mod external;
mod large_values;
mod links;
mod native_options;
mod native_target;
mod number;
use crate::output;
mod args;
mod parameters;
mod preview_calculation;
mod retrieval;
mod run_args;
mod saved;
mod text_policy;
pub(crate) use args::Command;
use run_args::RunArgs;
use std::fmt;
use tablecraft_access::{
    CalculationError, ExternalSourceError, LinkSourceError, template::TemplateError,
};
use tablecraft_expressions::EvalError;

pub(super) enum Error {
    Input(tablecraft_access::AccessInputError),
    Links(LinkSourceError),
    External(ExternalSourceError),
    Calculation(CalculationError),
    Template(TemplateError),
    Parse(tablecraft_sql::ParseError),
    Unsupported,
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
    Copy(editing::CopyError),
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
            Self::Parameter => f.write_str("invalid query run parameter"),
        }
    }
}
pub(super) fn run(command: Command, password: Option<&str>) -> Result<String, Error> {
    match command {
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
        Command::Run(arguments) => {
            let RunArgs {
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
                sql,
                saved,
                parameters,
                collation,
                like_mode,
                database,
                links,
                external,
                external_index,
                native_bytes,
                native_rows,
                native_steps,
                execution_limits,
                comparison_limits,
                text_policy,
                declared_integral,
                large_values,
                multivalues,
                collections,
                decoded_attachments,
                stored_calculations,
                attachment_bytes,
                native_value_bytes,
                native_text_bytes,
                sqlite,
                index,
                auto_index,
            } = *arguments;
            let parsed = sql.as_deref().map(parse).transpose()?;
            let input = match (parsed.as_ref(), saved.as_deref()) {
                (Some(query), None) => retrieval::Input::Sql(query.query()),
                (None, Some(name)) => retrieval::Input::Saved(name),
                _ => return Err(Error::Parameter),
            };
            retrieval::run(
                input,
                &parameters,
                retrieval::Controls {
                    declared_integral,
                    max_text_bytes: large_values.text_bytes.map(|bytes| bytes as usize),
                    output_bytes: large_values
                        .output_bytes
                        .map_or(crate::output::MAX_OUTPUT_BYTES, |bytes| bytes as usize),
                    execution_limits: execution_limits.limits(),
                    comparison_limits,
                    text_policy,
                    collation,
                    like_mode,
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
                database.as_deref().map(|path| retrieval::NativeInput {
                    path,
                    links: &links,
                    external: &external,
                    external_index: &external_index,
                    bytes: native_bytes.unwrap_or(crate::native::DEFAULT_NATIVE_BYTES),
                    rows: native_rows.unwrap_or(crate::native::DEFAULT_NATIVE_ROWS),
                    steps: native_steps.unwrap_or(crate::native::DEFAULT_NATIVE_STEPS),
                    index: &index,
                    auto_index,
                    multivalues,
                    collections,
                    decoded_attachments,
                    stored_calculations,
                    attachment_bytes,
                    native_value_bytes,
                    native_text_bytes,
                    password,
                }),
                sqlite.as_deref(),
            )
        }
        Command::Update {
            sql,
            database,
            output,
            collation,
        } => editing::run(
            parse(&sql)?.query(),
            &database,
            &output,
            collation.map(Into::into),
        ),
    }
}
fn parse(sql: &str) -> Result<tablecraft_sql::ParsedQuery, Error> {
    tablecraft_sql::parse_query(sql, tablecraft_sql::ParseLimits::default()).map_err(Error::Parse)
}
