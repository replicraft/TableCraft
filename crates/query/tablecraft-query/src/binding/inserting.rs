//! VALUES has declared inputs but no relation, identity or retrieval capability.
use super::{Binder, Parameter};
use crate::{QueryError, Source, plan::Expr};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::Column;
use tablecraft_sql::query::{InsertSource, Query, Statement};

pub(crate) struct ValuesSource;

impl Source for ValuesSource {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        Err(QueryError::Unsupported)
    }

    fn scan(
        &self,
        _table: &str,
        _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        Err(QueryError::Unsupported)
    }
}

pub(crate) struct BoundInsertValues {
    pub parameters: Vec<Parameter>,
    pub expressions: Vec<Expr>,
}

pub(crate) fn bind_insert_values(
    query: &Query,
    source: &ValuesSource,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BoundInsertValues, QueryError> {
    let Statement::Insert(insert) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let InsertSource::Values(values) = &insert.source else {
        return Err(QueryError::Unsupported);
    };
    super::mutation_parameters::admit(&query.parameters, Some(budget), cancelled)?;
    let mut binder = Binder::mutation(source);
    binder.parameters = binder.bind_parameters(&query.parameters)?;
    binder.parameter_scope = binder.scope_indices(binder.parameters.len())?;
    budget.charge_allocation(values.len() as u64, size_of::<Expr>() as u64)?;
    let mut expressions = Vec::new();
    expressions
        .try_reserve_exact(values.len())
        .map_err(|_| QueryError::Allocation)?;
    for expression in values {
        if cancelled() {
            return Err(QueryError::Cancelled);
        }
        budget.charge(Resource::Steps, 1)?;
        let value = binder.expression(expression, &[], false, 1)?;
        if !super::mutation_scalar::admitted(&value) {
            return Err(QueryError::Unsupported);
        }
        expressions.push(value);
    }
    Ok(BoundInsertValues {
        parameters: binder.parameters,
        expressions,
    })
}
