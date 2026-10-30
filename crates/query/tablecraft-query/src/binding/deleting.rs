//! Bind a single local DELETE predicate without enabling hidden retrieval.
use super::Binder;
use crate::{QueryError, Source, plan::Expr};
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

pub(crate) struct BoundDelete {
    pub filter: Option<Expr>,
}

pub(crate) fn bind_delete(query: &Query, source: &dyn Source) -> Result<BoundDelete, QueryError> {
    if !query.parameters.is_empty() {
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
    let qualifier = selected.alias.as_ref().unwrap_or(&selected.name).as_str();
    if !delete.targets.is_empty()
        && (delete.targets.len() != 1
            || !delete.targets[0].as_str().eq_ignore_ascii_case(qualifier))
    {
        return Err(QueryError::Unsupported);
    }

    let mut binder = Binder {
        inherited_read_depth: 0,
        pending_named_depth: 0,
        next_relation_occurrence: 0,
        binding_depth: Default::default(),
        aliases: Default::default(),
        budget: None,
        cancelled: None,
        source,
        external_sources: Vec::new(),
        external_providers: Vec::new(),
        materialized: &[],
        discover: false,
        reference_year: None,
        declared_arithmetic: false,
        pending_schema: None,
        parameters: Vec::new(),
        parameter_scope: Vec::new(),
        retrieval: false,
        mutation_scalar: true,
        collection_criteria: false,
        active_queries: Vec::new(),
        owner: crate::external_sources::SourceNamespace::Root,
        nodes: 0,
        policy_visits: 0,
        bytes: 0,
    };
    let relation = binder.relation(
        table,
        &[],
        1,
        crate::external_sources::SourceNamespace::Root,
    )?;

    let scopes = [relation.slots.as_slice()];
    let filter = delete
        .selection
        .as_ref()
        .map(|filter| binder.expression(filter, &scopes, false, 1))
        .transpose()?;
    if filter
        .as_ref()
        .is_some_and(|filter| !super::mutation_scalar::admitted(filter))
    {
        return Err(QueryError::Unsupported);
    }

    Ok(BoundDelete { filter })
}
