use super::{Error, output::TableOutput, parameters};
use crate::native;
use clap::ValueEnum;
use std::cmp::Ordering;
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_access::NativeSource;
use tablecraft_core::values::Value;
use tablecraft_expressions::functions::dates::TwoDigitYearWindow;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{ExecutionOptions, QueryResult, Source, prepare, source::MemorySource};
use tablecraft_sql::query::Query;

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum TextCollation {
    /// Unicode ordinal order, case-sensitive; no normalization or locale rules.
    Ordinal,
}
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
impl Collation for TextCollation {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        match self {
            Self::Ordinal => Ok(left.cmp(right)),
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

pub(super) fn run(
    input: Input<'_>,
    raw: &[String],
    collation: Option<TextCollation>,
    like_mode: Option<Wildcards>,
    year_window: Option<TwoDigitYearWindow>,
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
        let result = execute(
            input.sql()?,
            &source,
            &bindings,
            collation.as_ref(),
            like_mode,
            year_window,
        )?;
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
        execute(
            query,
            &source,
            &bindings,
            collation.as_ref(),
            like_mode,
            year_window,
        )?
    } else {
        execute(
            input.sql()?,
            &MemorySource::default(),
            &bindings,
            collation.as_ref(),
            like_mode,
            year_window,
        )?
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
    collation: Option<&TextCollation>,
    like_mode: Option<Wildcards>,
    year_window: Option<TwoDigitYearWindow>,
) -> Result<QueryResult, Error> {
    let plan = prepare(query, source).map_err(Error::Retrieval)?;
    plan.execute(
        source,
        bindings,
        ExecutionOptions {
            year_window,
            like_mode: like_mode.map(Into::into),
            collation: collation.map(|c| c as &dyn Collation),
            ..Default::default()
        },
    )
    .map_err(Error::Retrieval)
}
