use super::{Error, output::TableOutput, parameters};
use clap::ValueEnum;
use std::cmp::Ordering;
use std::{fs::File, path::Path};
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryResult, Source, prepare, source::MemorySource};
use tablecraft_sql::query::Query;

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum TextCollation {
    /// Unicode ordinal order, case-sensitive; no normalization or locale rules.
    Ordinal,
}
impl Collation for TextCollation {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        match self {
            Self::Ordinal => Ok(left.cmp(right)),
        }
    }
}
pub(super) fn run(
    query: &Query,
    raw: &[String],
    collation: Option<TextCollation>,
    database: Option<&Path>,
) -> Result<String, Error> {
    let parameters = parameters::parse(raw)?;
    let bindings = parameters
        .iter()
        .map(|(name, value)| (name.as_str(), value.clone()))
        .collect::<Vec<_>>();
    let result = if let Some(path) = database {
        let file = File::open(path).map_err(|e| Error::Native(e.into()))?;
        let source = NativeSource::new(
            file,
            CatalogScope::new(1),
            ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
            &|| false,
        )
        .map_err(Error::Native)?;
        execute(query, &source, &bindings, collation.as_ref())?
    } else {
        execute(
            query,
            &MemorySource::default(),
            &bindings,
            collation.as_ref(),
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
) -> Result<QueryResult, Error> {
    let plan = prepare(query, source).map_err(Error::Retrieval)?;
    plan.execute(
        source,
        bindings,
        ExecutionOptions {
            collation: collation.map(|c| c as &dyn Collation),
            ..Default::default()
        },
    )
    .map_err(Error::Retrieval)
}
