//! Buffered query execution over the retained native source, without provider opens.
pub(crate) mod context;
pub(crate) mod inspection;
pub(crate) mod options;
pub(crate) mod parameters;

use super::{
    Cell, Error, QueryRunOptions, Response,
    controls::{SCOPE, check},
    display::Presentation,
    document::Document,
};
use std::cell::RefCell;
use tablecraft_access::{NativeCollationResolver, NativeSource};
use tablecraft_ace::{DatabaseQueryMode, NativeReadSource};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{functions::dates::clock::Clock, predicates::LikeMode};
use tablecraft_query::{ExecutionOptions, execute_with_schema_discovery};
use tablecraft_sql::{ParseLimits, parse_query, query::Query};

const SQL_BYTES: usize = 64 * 1024;

pub(super) struct RunSettings<'a> {
    pub options: QueryRunOptions,
    pub title: &'a str,
    pub clock: &'a dyn Clock,
    pub cancelled: &'a dyn Fn() -> bool,
}

impl Document {
    pub(super) fn query(
        &mut self,
        sql: &str,
        options: QueryRunOptions,
        clock: &dyn Clock,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if sql.len() > SQL_BYTES {
            return Err(Error::SqlLimit);
        }
        let parsed = parse_query(sql, ParseLimits::default())?;
        let source = NativeSource::new_with_snapshot_limits(
            &mut self.source,
            SCOPE,
            options.native().budget(),
            cancelled,
            options.snapshot(),
        )
        .map_err(options::native_error)?;
        let comparison_pool = RefCell::new(options.comparison().budget());
        let (mut source, resolver) =
            source.with_deferred_database_collation_resolver(&comparison_pool)?;
        execute(
            parsed.query(),
            &mut source,
            &resolver,
            &[],
            RunSettings {
                options,
                title: "Query results",
                clock,
                cancelled,
            },
            Presentation::with_budget(options.presentation().budget(), cancelled),
        )
    }
}

pub(super) fn execute<R: NativeReadSource>(
    query: &Query,
    source: &mut NativeSource<'_, R>,
    resolver: &NativeCollationResolver<'_>,
    parameters: &[(&str, Value)],
    settings: RunSettings<'_>,
    output: Presentation<'_>,
) -> Result<Response, Error> {
    let RunSettings {
        options,
        title,
        clock,
        cancelled,
    } = settings;
    let context = context::Context::new(clock);
    let like_mode = match source
        .database_query_mode()
        .map_err(options::native_error)?
    {
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
            limits: options.query().query_limits(),
            collation: Some(resolver.default_collation()),
            collation_resolver: Some(resolver),
            like_mode,
            ..context.options()
        },
    )
    .map_err(options::query_error)?;
    present(result, title, output).map_err(options::presentation_error)
}

fn present(
    result: tablecraft_query::ExpressionQueryResult,
    title: &str,
    mut output: Presentation<'_>,
) -> Result<Response, Error> {
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
