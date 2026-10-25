//! Explicit progressive row-local execution; ordinary retrieval stays buffered.
mod admission;

use super::{
    Environment, ExecutionContext, ExecutionOptions, cell::into_scalar, execute::ExecutionFrame,
};
use crate::{
    PreparedQuery, QueryError, Source,
    discovery::DiscoveryState,
    plan::{PreparedBody, RelationNode},
    source::ProjectedRow,
};
use std::fmt;
use tablecraft_core::values::Value;

/// Counts returned only after the progressive provider reports successful EOF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamSummary {
    /// Successfully validated input rows, including rows rejected by WHERE.
    pub scanned_rows: u64,
    /// Complete output rows accepted by the visitor.
    pub emitted_rows: u64,
}

/// A progressive execution failure; earlier accepted rows remain published.
#[derive(Debug)]
pub enum StreamError<E> {
    /// Admission, schema, source, evaluation, cancellation or resource failure.
    Query(QueryError),
    /// The original visitor error, retained even if a source swallows rejection.
    Sink(E),
}

impl<E> From<QueryError> for StreamError<E> {
    fn from(error: QueryError) -> Self {
        Self::Query(error)
    }
}

impl<E: fmt::Display> fmt::Display for StreamError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Query(error) => error.fmt(formatter),
            Self::Sink(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for StreamError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Query(error) => Some(error),
            Self::Sink(error) => Some(error),
        }
    }
}

impl PreparedQuery {
    /// Progressively evaluate a direct local scalar SELECT/WHERE in source order.
    /// Sorts, TOP, sets, groups, joins, derived/named sources, collections,
    /// subqueries, domain calls, Eval and volatile functions are refused before
    /// scanning, even for empty inputs. The source must explicitly implement
    /// progressive projection; this never falls back to buffered retrieval.
    ///
    /// Each row retains ordinary eager predicate/projection and strict scalar
    /// publication rules. A later read/evaluation/EOF failure can follow already
    /// accepted rows. Cumulative budgets never refill and no result cache is
    /// published. Cross-row failure precedence follows progressive encounter
    /// order, rather than a buffered provider's load-before-evaluation order.
    ///
    /// Providers may retain exclusive cursor access across callbacks. The visitor
    /// and supplied expression providers must not reenter the source. The first
    /// visitor error stops evaluation and is preserved even if a broken source
    /// continues callbacks or returns another error. Metadata is available from
    /// the prepared query's output_names/output_types accessors.
    pub fn execute_progressive<'a, E>(
        &self,
        source: &'a dyn Source,
        parameters: &'a [(&str, Value)],
        options: ExecutionOptions<'a>,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<StreamSummary, StreamError<E>> {
        let _span = tracing::debug_span!("query.execute_progressive").entered();
        // This first slice never resolves external namespaces, even if a bound
        // but unused dependency happened to capture one.
        if !self.external_sources.is_empty() {
            return Err(QueryError::Unsupported.into());
        }
        let mut budget = crate::discovery::budget(options.limits);
        let discovery = DiscoveryState::new(false);
        let mut runtime = self.initialize_runtime(
            ExecutionFrame {
                source,
                parameters,
                options,
                discovery: &discovery,
                context: ExecutionContext {
                    strict_parameters: true,
                    repeatable_dynamic: false,
                },
            },
            &mut budget,
        )?;
        let select = runtime.progressive_select(&self.body)?;
        runtime.check_table_schema(&select.source.node)?;
        runtime.tick()?;
        let RelationNode::Table {
            name,
            columns,
            projection,
            ..
        } = &select.source.node
        else {
            return Err(QueryError::Unsupported.into());
        };
        let PreparedBody::Read(read) = &self.body else {
            return Err(QueryError::Unsupported.into());
        };
        let mut summary = StreamSummary {
            scanned_rows: 0,
            emitted_rows: 0,
        };
        let mut failure: Option<QueryError> = None;
        let mut sink_failure = None;
        let mut consume = |row: ProjectedRow<'_>| {
            if let Some(error) = &failure {
                return Err(error.clone());
            }
            let result = (|| {
                runtime.tick()?;
                let row = runtime.copy_projected_row(columns, projection, row)?;
                summary.scanned_rows = summary
                    .scanned_rows
                    .checked_add(1)
                    .ok_or(QueryError::StructureLimit)?;
                let environment = Environment {
                    row: &row,
                    collections: None,
                    outer: &[],
                    output: &[],
                    group: None,
                    group_keys: None,
                };
                if let Some(filter) = &select.filter
                    && !runtime.eval_predicate(filter, environment)?.is_true()
                {
                    return Ok(());
                }
                let mut cells = runtime.allocate_expression_row(select.projection.len())?;
                for expression in &select.projection {
                    cells.push(runtime.retained_expression_operand(expression, environment)?);
                }
                let mut values = runtime.allocate_row(cells.len())?;
                for mut cell in cells {
                    runtime.tick()?;
                    cell.value = runtime.evaluator().publish_expression_value(cell.value)?;
                    values.push(into_scalar(cell)?);
                }
                if values.len() != read.names.len() {
                    return Err(QueryError::Width);
                }
                runtime.tick()?;
                if let Err(error) = visitor(&values) {
                    sink_failure = Some(error);
                    return Err(QueryError::Source);
                }
                summary.emitted_rows = summary
                    .emitted_rows
                    .checked_add(1)
                    .ok_or(QueryError::StructureLimit)?;
                Ok(())
            })();
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            result
        };
        let scanned = source.scan_projected_progressive(name, projection, &mut consume);
        if let Some(error) = sink_failure {
            return Err(StreamError::Sink(error));
        }
        if let Some(error) = failure {
            return Err(StreamError::Query(error));
        }
        scanned?;
        runtime.tick()?;
        tracing::debug!(
            scanned_rows = summary.scanned_rows,
            emitted_rows = summary.emitted_rows,
            "progressive query completed"
        );
        Ok(summary)
    }
}
