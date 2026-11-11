//! Complete make-table query staging; native table creation remains a host operation.
mod declaration;

use super::{ExecutionOptions, result::publish_cells};
use crate::{QueryError, Source, discovery::execute_staged_source};
pub use declaration::MakeTableColumn;
use std::mem;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Value, ValueError},
};
use tablecraft_sql::{
    query::{Query, ReadBody, ReadQuery, Statement},
    select::Name,
};
use thiserror::Error;

/// A make-table staging failure, without SQL, names or value payloads.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MakeTableError {
    /// The owned query is not SELECT INTO or has no destination.
    #[error("expected a make-table query")]
    Statement,
    /// External destination publication requires a separate authorized host contract.
    #[error("external make-table destination is not supported")]
    ExternalDestination,
    /// The source's table/query namespace already exposes this local name.
    #[error("make-table destination already exists")]
    DestinationExists,
    /// A NULL-only or dynamic output has no established stored scalar declaration.
    #[error("make-table output type is not established")]
    IndeterminateType,
    /// Whole lists/recordsets have no ordinary scalar destination declaration.
    #[error("make-table output is not scalar")]
    StructuredOutput,
    /// Ordinary query, metadata, schema, resource or cancellation failure.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// Expression-only states cannot silently become stored NULL or another scalar.
    #[error(transparent)]
    Value(#[from] ValueError),
}

/// One complete SELECT INTO evaluation, not native creation or save authority.
/// Output declarations describe query values, not native field sizes/properties.
/// The host must independently resolve physical declarations, collision rules,
/// source stability and publication through its actual reviewed creation owner.
#[must_use = "resolve declarations and consume or explicitly discard the staged rows"]
pub struct MakeTablePlan {
    destination: Name,
    retrieval: Query,
    columns: Vec<MakeTableColumn>,
    rows: Vec<Vec<Value>>,
}

impl MakeTablePlan {
    /// Requested local table name; no existing object is replaced by this plan.
    pub fn destination(&self) -> &str {
        self.destination.as_str()
    }

    /// Moved original SELECT/parameters without INTO. Hosts can resolve actual
    /// source field sizes from this context; no source or native handles are owned.
    pub fn retrieval(&self) -> &Query {
        &self.retrieval
    }

    /// Static query declarations retained even when no records were selected.
    pub fn columns(&self) -> &[MakeTableColumn] {
        &self.columns
    }

    /// Complete stored scalar rows. NULL is never filled from a source default.
    pub fn rows(&self) -> &[Vec<Value>] {
        &self.rows
    }

    /// Move the complete rows after the host has resolved its own declarations.
    /// No payload copy or additional evaluation occurs.
    pub fn into_rows(self) -> Vec<Vec<Value>> {
        self.rows
    }
}

/// Evaluate one owned local SELECT INTO under a single cumulative query budget.
/// Existing destination metadata is checked before any user-record scan. Unknown
/// names alone permit staging; other metadata failures propagate. Original source
/// references, parameters, expression declarations and execution providers are
/// preserved. This neither creates a table nor infers native storage from rows.
/// Output declaration validation follows complete evaluation, including named
/// schema discovery; a refused declaration may therefore follow source reads.
/// No partial result or creation capability is returned on that failure.
///
/// # Errors
/// Refuses non-make-table/external destinations, existing targets, duplicate or
/// invalid output names, unresolved/structured types, non-storable values, query
/// failures, cancellation, caller exhaustion and fallible buffer allocation.
pub fn plan_make_table<'a>(
    query: Query,
    source: &'a dyn Source,
    budget: &mut ResourceBudget,
    parameters: &'a [(&str, Value)],
    options: ExecutionOptions<'a>,
) -> Result<MakeTablePlan, MakeTableError> {
    let _span = tracing::debug_span!("query.make_table.prepare").entered();
    check(budget, options.cancelled)?;
    let Query {
        parameters: declarations,
        statement,
    } = query;
    let Statement::MakeTable(mut select) = statement else {
        return Err(MakeTableError::Statement);
    };
    let destination = select.into.take().ok_or(MakeTableError::Statement)?;
    if select.into_external.is_some() {
        return Err(MakeTableError::ExternalDestination);
    }
    budget
        .charge(Resource::Steps, destination.as_str().len() as u64)
        .map_err(QueryError::from)?;
    match source.columns(destination.as_str()) {
        Ok(_) => return Err(MakeTableError::DestinationExists),
        Err(QueryError::UnknownName) => {}
        Err(error) => return Err(error.into()),
    }
    check(budget, options.cancelled)?;
    if source.query(destination.as_str())?.is_some() {
        return Err(MakeTableError::DestinationExists);
    }
    check(budget, options.cancelled)?;
    budget
        .charge_allocation(1, size_of::<ReadQuery>() as u64)
        .map_err(QueryError::from)?;
    let retrieval = Query {
        parameters: declarations,
        statement: Statement::Read(Box::new(ReadQuery {
            order_by: mem::take(&mut select.order_by),
            owner_access: mem::replace(&mut select.owner_access, false),
            body: ReadBody::Select(select),
        })),
    };
    let result = execute_staged_source(&retrieval, source, parameters, options, budget, None)?;
    let result = publish_cells(result, budget, options)?;
    let columns = declaration::columns(
        result.columns,
        &result.output_types,
        budget,
        options.cancelled,
    )?;
    let mut rows = Vec::new();
    budget
        .charge_allocation(result.rows.len() as u64, size_of::<Vec<Value>>() as u64)
        .map_err(QueryError::from)?;
    rows.try_reserve_exact(result.rows.len())
        .map_err(|_| QueryError::Allocation)?;
    for row in result.rows {
        check(budget, options.cancelled)?;
        if row.len() != columns.len() {
            return Err(QueryError::Width.into());
        }
        budget
            .charge_allocation(row.len() as u64, size_of::<Value>() as u64)
            .map_err(QueryError::from)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(row.len())
            .map_err(|_| QueryError::Allocation)?;
        for (value, column) in row.into_iter().zip(&columns) {
            check(budget, options.cancelled)?;
            let value = Value::try_from(value)?;
            declaration::validate(column, &value, budget, options.cancelled)?;
            values.push(value);
        }
        rows.push(values);
    }
    check(budget, options.cancelled)?;
    Ok(MakeTablePlan {
        destination,
        retrieval,
        columns,
        rows,
    })
}

fn check(
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(), QueryError> {
    if cancelled.is_some_and(|cancel| cancel()) {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

#[cfg(test)]
mod tests;
