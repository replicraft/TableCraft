//! Buffered query execution over the retained native source, without provider opens.
pub(crate) mod context;

use super::{
    Cell, Error, Response,
    controls::{SCOPE, budget, check, comparison_budget},
    display::Presentation,
    document::Document,
};
use std::cell::RefCell;
use tablecraft_access::{NativeCollationResolver, NativeSource};
use tablecraft_ace::{DatabaseQueryMode, NativeReadSource};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{functions::dates::clock::Clock, predicates::LikeMode};
use tablecraft_query::{ExecutionOptions, QueryLimits, execute_with_schema_discovery};
use tablecraft_sql::{ParseLimits, parse_query, query::Query};

const SQL_BYTES: usize = 64 * 1024;
const QUERY_BYTES: u64 = 16 * 1024 * 1024;
const QUERY_ROWS: u64 = 10_000;
const QUERY_STEPS: u64 = 1_000_000;

impl Document {
    pub(super) fn query(
        &mut self,
        sql: &str,
        clock: &dyn Clock,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if sql.len() > SQL_BYTES {
            return Err(Error::SqlLimit);
        }
        let parsed = parse_query(sql, ParseLimits::default())?;
        let source = NativeSource::new(&mut self.source, SCOPE, budget(), cancelled)?;
        let comparison_pool = RefCell::new(comparison_budget());
        let (mut source, resolver) =
            source.with_deferred_database_collation_resolver(&comparison_pool)?;
        execute(
            parsed.query(),
            &mut source,
            &resolver,
            &[],
            "Query results",
            clock,
            cancelled,
        )
    }
}

pub(super) fn execute<R: NativeReadSource>(
    query: &Query,
    source: &mut NativeSource<'_, R>,
    resolver: &NativeCollationResolver<'_>,
    parameters: &[(&str, Value)],
    title: &str,
    clock: &dyn Clock,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let context = context::Context::new(clock);
    let like_mode = match source.database_query_mode()? {
        DatabaseQueryMode::Ansi89 => Some(LikeMode::Ansi89),
        DatabaseQueryMode::Ansi92 => Some(LikeMode::Ansi92),
        DatabaseQueryMode::Absent | DatabaseQueryMode::Unknown => None,
    };
    let result = execute_with_schema_discovery(
        query,
        source,
        parameters,
        ExecutionOptions {
            cancelled: Some(cancelled),
            limits: QueryLimits {
                bytes: QUERY_BYTES,
                rows: QUERY_ROWS,
                steps: QUERY_STEPS,
            },
            collation: Some(resolver.default_collation()),
            collation_resolver: Some(resolver),
            like_mode,
            ..context.options()
        },
    )?;
    let mut output = Presentation::new(cancelled);
    let mut columns = output.columns(result.columns().len())?;
    for name in result.columns() {
        columns.push(crate::column::plain(name, &mut output)?);
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
