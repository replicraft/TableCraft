//! Explicit progressive row-local execution; ordinary retrieval stays buffered.
mod admission;
mod aggregates;
mod groups;
mod plan;
mod publication;
mod rows;

use super::{ExecutionContext, ExecutionOptions, execute::ExecutionFrame};
use crate::{PreparedQuery, QueryError, Source, discovery::DiscoveryState};
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
    /// Progressively evaluate local scalar SELECT/WHERE arms in written source order.
    /// Unordered positive TOP counts limit publication while all rows are still
    /// read and evaluated through checked EOF. Flat UNION ALL combines admitted
    /// arms through the ordinary UNION result conversions. Global and proven
    /// Boolean/integral/NULL grouped Count, Sum, Avg, Min and Max retain one state
    /// per occurrence and group. Group keys and aggregate operands run per row;
    /// checked EOF precedes final projections. Min/Max retain resolver-dependent
    /// String candidates under the cumulative budget. Other aggregates are refused.
    /// Sorts, TOP PERCENT,
    /// duplicate-eliminating sets, other grouping keys, HAVING,
    /// joins, derived/named sources, collections,
    /// subqueries, domain calls, Eval and volatile functions are refused before
    /// scanning, even for empty inputs. The source must explicitly implement
    /// progressive projection; this never falls back to buffered retrieval.
    ///
    /// Each row retains ordinary eager predicate/projection and strict scalar
    /// publication rules. A later read/evaluation/EOF failure can follow already
    /// accepted rows. Cumulative budgets never refill and no result cache is
    /// published. Cross-row failure precedence follows progressive encounter
    /// order, rather than a buffered provider's load-before-evaluation order.
    /// Group finalization/projection errors may follow earlier accepted groups;
    /// EOF or input-operand failure prevents every result from that arm.
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
        let mut plan = runtime.progressive_plan(&self.body)?;
        let mut summary = StreamSummary {
            scanned_rows: 0,
            emitted_rows: 0,
        };
        for arm in &mut plan.arms {
            runtime.progressive_rows(arm, plan.union_types, &mut summary, visitor)?;
        }
        runtime.tick()?;
        tracing::debug!(
            scanned_rows = summary.scanned_rows,
            emitted_rows = summary.emitted_rows,
            "progressive query completed"
        );
        Ok(summary)
    }
}
