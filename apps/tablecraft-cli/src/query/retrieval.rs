use super::{Error, output::TableOutput, parameters};
use clap::ValueEnum;
use std::cmp::Ordering;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
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
) -> Result<String, Error> {
    let parameters = parameters::parse(raw)?;
    let bindings = parameters
        .iter()
        .map(|(name, value)| (name.as_str(), value.clone()))
        .collect::<Vec<_>>();
    let source = MemorySource::default();
    let plan = prepare(query, &source).map_err(Error::Retrieval)?;
    let result = plan
        .execute(
            &source,
            &bindings,
            ExecutionOptions {
                collation: collation.as_ref().map(|c| c as &dyn Collation),
                ..Default::default()
            },
        )
        .map_err(Error::Retrieval)?;
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
