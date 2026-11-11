//! Bind a single local DELETE predicate without enabling hidden retrieval.
use super::{Binder, Parameter};
use crate::{QueryError, Source, plan::Expr};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

pub(crate) mod read_source;

pub(crate) struct BoundDelete {
    pub parameters: Vec<Parameter>,
    pub filter: Option<Expr>,
}

pub(crate) fn bind_delete(
    query: &Query,
    source: &dyn Source,
    allow_parameters: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BoundDelete, QueryError> {
    bind_with(query, source, allow_parameters, budget, cancelled, None)
}

fn bind_with(
    query: &Query,
    source: &dyn Source,
    allow_parameters: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    target_columns: Option<(&str, &[tablecraft_schema::Column])>,
) -> Result<BoundDelete, QueryError> {
    if !allow_parameters && !query.parameters.is_empty() {
        return Err(QueryError::Unsupported);
    }

    let Statement::Delete(delete) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let [table @ TableSource::Table(selected)] = delete.sources.as_slice() else {
        return Err(QueryError::Unsupported);
    };
    if selected.database.is_some() {
        return Err(QueryError::Unsupported);
    }
    if let Some((expected, _)) = target_columns
        && !selected.name.as_str().eq_ignore_ascii_case(expected)
    {
        return Err(QueryError::Source);
    }
    let qualifier = selected.alias.as_ref().unwrap_or(&selected.name).as_str();
    if !delete.targets.is_empty()
        && (delete.targets.len() != 1
            || !delete.targets[0].as_str().eq_ignore_ascii_case(qualifier))
    {
        return Err(QueryError::Unsupported);
    }

    if allow_parameters {
        super::mutation_parameters::admit(&query.parameters, Some(budget), cancelled)?;
    }

    let mut binder = Binder::mutation(source, budget, cancelled);

    binder.parameters = binder.bind_parameters(&query.parameters)?;
    binder.parameter_scope = binder.scope_indices(binder.parameters.len())?;

    let relation = binder.relation(
        table,
        &[],
        1,
        crate::external_sources::SourceNamespace::Root,
    )?;

    if let Some((_, expected)) = target_columns {
        let crate::plan::RelationNode::Table {
            columns,
            multivalues,
            members,
            recordsets,
            ..
        } = &relation.node
        else {
            return Err(QueryError::Unsupported);
        };
        if !multivalues.is_empty() || !members.is_empty() || !recordsets.is_empty() {
            return Err(QueryError::Unsupported);
        }
        if columns != expected {
            return Err(QueryError::SchemaChanged);
        }
    }
    let scopes = [relation.slots.as_slice()];
    let filter = delete
        .selection
        .as_ref()
        .map(|filter| binder.expression(filter, &scopes, false, 1))
        .transpose()?;
    if let Some(filter) = &filter {
        if target_columns.is_some() {
            super::mutation_reads::admit(filter, &mut binder)?;
        } else if !super::mutation_scalar::admitted(filter) {
            return Err(QueryError::Unsupported);
        }
    }

    Ok(BoundDelete {
        parameters: binder.parameters,
        filter,
    })
}
