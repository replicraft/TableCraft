use super::{Error, output::TableOutput, parameters};
use crate::native;
use clap::ValueEnum;
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_access::NativeSource;
use tablecraft_core::values::Value;
use tablecraft_expressions::comparison::Collation;
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_expressions::functions::text::cstr::DateTextFormat;
use tablecraft_expressions::functions::{
    dates::TwoDigitYearWindow, text::code_page::CharacterCodePage,
};
use tablecraft_query::{ExecutionOptions, QueryResult, Source, prepare, source::MemorySource};
use tablecraft_sql::query::Query;

pub(crate) use super::collation::TextCollation;
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Wildcards {
    /// Access asterisk, question mark, digit and negated-list wildcards.
    Ansi89,
    /// SQL-compatible percent, underscore and negated-list wildcards.
    Ansi92,
}
impl From<Wildcards> for tablecraft_expressions::predicates::LikeMode {
    fn from(mode: Wildcards) -> Self {
        match mode {
            Wildcards::Ansi89 => Self::Ansi89,
            Wildcards::Ansi92 => Self::Ansi92,
        }
    }
}
pub(super) enum Input<'a> {
    Sql(&'a Query),
    Saved(&'a str),
}
impl<'a> Input<'a> {
    fn sql(&self) -> Result<&'a Query, Error> {
        match self {
            Self::Sql(query) => Ok(query),
            Self::Saved(_) => Err(Error::Parameter),
        }
    }
}

pub(super) struct NativeInput<'a> {
    pub password: Option<&'a str>,
    pub path: &'a Path,
    pub bytes: u64,
    pub index: &'a [String],
    pub multivalues: bool,
    pub collections: bool,
}

pub(super) struct Controls {
    pub collation: Option<TextCollation>,
    pub like_mode: Option<Wildcards>,
    pub year_window: Option<TwoDigitYearWindow>,
    pub character_code_page: Option<CharacterCodePage>,
    pub number_format: Option<NumberFormat>,
    pub date_format: Option<DateTextFormat>,
    pub first_day_of_week: Option<tablecraft_expressions::functions::dates::FirstDayOfWeek>,
    pub reference_year: Option<tablecraft_expressions::functions::dates::parsing::ReferenceYear>,
}

pub(super) fn run(
    input: Input<'_>,
    raw: &[String],
    controls: Controls,
    native_input: Option<NativeInput<'_>>,
    sqlite: Option<&Path>,
) -> Result<String, Error> {
    let parameters = parameters::parse(raw)?;
    let bindings = parameters
        .iter()
        .map(|(name, value)| (name.as_str(), value.clone()))
        .collect::<Vec<_>>();
    let result = if let Some(path) = sqlite {
        let source = tablecraft_sqlite::SqliteSource::open(
            path,
            native::budget(),
            Arc::new(AtomicBool::new(false)),
        )
        .map_err(Error::Sqlite)?;
        let result = execute(input.sql()?, &source, &bindings, &controls)?;
        source.close().map_err(Error::Sqlite)?;
        result
    } else if let Some(input_config) = native_input {
        let mut budget = native::read_budget(input_config.bytes);
        let file = native::open(input_config.path, input_config.password, &mut budget)
            .map_err(Error::Input)?;
        let source = NativeSource::new(file, native::CATALOG_SCOPE, budget, &|| false)
            .map_err(Error::Native)?;
        let source = if input_config.collections {
            source.with_collections()
        } else if input_config.multivalues {
            source.with_multivalues()
        } else {
            source
        };
        let mut source = match input_config.index {
            [] => source,
            [table, index] => source.with_index(table, index).map_err(Error::Native)?,
            _ => return Err(Error::Parameter),
        };
        let saved = match input {
            Input::Saved(name) => Some(
                source
                    .read_saved_select(name, Default::default())
                    .map_err(Error::SavedRead)?,
            ),
            Input::Sql(_) => None,
        };
        let query = match &saved {
            Some(saved) => saved.query(),
            None => input.sql()?,
        };
        execute(query, &source, &bindings, &controls)?
    } else {
        execute(input.sql()?, &MemorySource::default(), &bindings, &controls)?
    };
    let mut output = TableOutput::new();
    for (i, name) in result.columns().iter().enumerate() {
        if i > 0 {
            output.separator().map_err(Error::Output)?;
        }
        output.header(Some(name), i + 1).map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;
    for row in result.rows() {
        for (i, value) in row.iter().enumerate() {
            if i > 0 {
                output.separator().map_err(Error::Output)?;
            }
            output.value(value).map_err(Error::Output)?;
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(output.finish())
}

fn execute(
    query: &Query,
    source: &dyn Source,
    bindings: &[(&str, Value)],
    controls: &Controls,
) -> Result<QueryResult, Error> {
    let plan = prepare(query, source).map_err(Error::Retrieval)?;
    plan.execute(
        source,
        bindings,
        ExecutionOptions {
            year_window: controls.year_window,
            character_code_page: controls.character_code_page,
            number_format: controls.number_format,
            date_format: controls.date_format,
            reference_year: controls.reference_year,
            first_day_of_week: controls.first_day_of_week,
            like_mode: controls.like_mode.map(Into::into),
            collation: controls.collation.as_ref().map(|c| c as &dyn Collation),
            ..Default::default()
        },
    )
    .map_err(Error::Retrieval)
}
