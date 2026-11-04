//! Buffered query execution over the retained native source, without provider opens.
use super::{
    Cell, Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
};
use tablecraft_access::NativeSource;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_query::{ExecutionOptions, QueryLimits, Source, execute_with_schema_discovery};
use tablecraft_sql::{ParseLimits, parse_query, query::Query};

const SQL_BYTES: usize = 64 * 1024;

impl Document {
    pub(super) fn query(
        &mut self,
        sql: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if sql.len() > SQL_BYTES {
            return Err(Error::SqlLimit);
        }
        let parsed = parse_query(sql, ParseLimits::default())?;
        let source = NativeSource::new(&mut self.source, SCOPE, budget(), cancelled)?;
        execute(parsed.query(), &source, &[], "Query results", cancelled)
    }
}

pub(super) fn execute(
    query: &Query,
    source: &dyn Source,
    parameters: &[(&str, Value)],
    title: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let result = execute_with_schema_discovery(
        query,
        source,
        parameters,
        ExecutionOptions {
            cancelled: Some(cancelled),
            limits: QueryLimits {
                bytes: 16 * 1024 * 1024,
                rows: 10_000,
                steps: 1_000_000,
            },
            ..Default::default()
        },
    )?;
    let mut output = Presentation::new(cancelled);
    let mut columns = output.columns(result.columns().len())?;
    for name in result.columns() {
        columns.push(output.text(name)?);
    }
    let mut rows = output.rows(result.rows().len())?;
    for values in result.rows() {
        let mut row = output.row(values.len())?;
        for value in values {
            let cell = match value {
                ExpressionValue::Value(value) => output.scalar(value, false)?,
                ExpressionValue::Empty => Cell::expression(output.text("Empty")?),
                ExpressionValue::Missing => Cell::expression(output.text("Missing")?),
                ExpressionValue::Error(code) => {
                    Cell::expression(output.text(&format!("#ERROR({code})"))?)
                }
                ExpressionValue::StringBytes(_) => return Err(Error::UnpublishedString),
            };
            row.push(cell);
        }
        rows.push(row);
    }
    Ok(Response::Data {
        source_revision: None,
        table: output.text(title)?,
        offset: 0,
        columns,
        rows,
        has_more: false,
    })
}
