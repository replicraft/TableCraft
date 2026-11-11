//! Buffered query execution over the retained native source, without provider opens.
pub(crate) mod contents;
pub(crate) mod context;
pub(crate) mod inspection;
pub(crate) mod options;
pub(crate) mod parameter_discovery;
pub(crate) mod parameters;

use super::{
    Cell, Error, QueryRunOptions, Response,
    controls::{SCOPE, check},
    display::Presentation,
    document::Document,
};
use contents::{QueryContentsBuilder, QueryResultIssuer};
use std::{cell::RefCell, sync::Arc};
use tablecraft_access::{NativeCollationResolver, NativeSource};
use tablecraft_ace::{DatabaseQueryMode, NativeReadSource};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{functions::dates::clock::Clock, predicates::LikeMode};
use tablecraft_query::{
    CollectionQueryResult, ExecutionOptions, QueryValue, execute_collections_with_schema_discovery,
};
use tablecraft_sql::{ParseLimits, parse_query, query::Query};

const SQL_BYTES: usize = 64 * 1024;

pub(super) struct RunSettings<'a> {
    pub options: QueryRunOptions,
    pub title: &'a str,
    pub clock: &'a dyn Clock,
    pub cancelled: &'a dyn Fn() -> bool,
    pub issuer: QueryResultIssuer,
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
        let issuer = self.query_result_issuer();
        let source = NativeSource::new_with_snapshot_limits(
            &mut self.source,
            SCOPE,
            options.native().budget(),
            cancelled,
            options.snapshot(),
        )
        .map_err(options::native_error)?
        .with_collections();
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
                issuer,
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
        issuer,
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
    let result = execute_collections_with_schema_discovery(
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
    present(result, title, output, issuer).map_err(options::presentation_error)
}

fn present(
    result: CollectionQueryResult,
    title: &str,
    mut output: Presentation<'_>,
    issuer: QueryResultIssuer,
) -> Result<Response, Error> {
    let mut contents = issuer.builder();
    let mut columns = output.columns(result.columns().len())?;
    for name in result.columns() {
        columns.push(crate::column::plain(name, &mut output)?);
    }
    let mut rows = output.rows(result.rows().len())?;
    for (row_index, values) in result.rows().iter().enumerate() {
        let mut row = output.row(values.len())?;
        for (column_index, value) in values.iter().enumerate() {
            let cell = match value {
                QueryValue::Scalar(value) => match value {
                    ExpressionValue::Value(value) => output.scalar(value, false)?,
                    ExpressionValue::Empty => Cell::expression(output.text("Empty")?),
                    ExpressionValue::Missing => Cell::expression(output.text("Missing")?),
                    ExpressionValue::Error(code) => {
                        Cell::expression(output.text(&format!("#ERROR({code})"))?)
                    }
                    ExpressionValue::StringBytes(_) => return Err(Error::UnpublishedString),
                },
                QueryValue::Recordset(value) => {
                    // NO_CLONE: The issued read-only result retains immutable
                    // child metadata after the query's temporary result drops.
                    contents.recordset_cell(
                        row_index,
                        column_index,
                        Arc::clone(value),
                        &mut output,
                    )?
                }
                QueryValue::AbsentRecordset => {
                    QueryContentsBuilder::absent_recordset_cell(&mut output)?
                }
                QueryValue::Multivalue(_) | QueryValue::AbsentMultivalue => {
                    return Err(tablecraft_query::QueryError::Unsupported.into());
                }
            };
            row.push(cell);
        }
        rows.push(row);
    }
    Ok(Response::Data {
        query_contents: contents.finish(&mut output)?,
        source_revision: None,
        table: output.text(title)?,
        offset: 0,
        columns,
        rows,
        has_more: false,
    })
}
