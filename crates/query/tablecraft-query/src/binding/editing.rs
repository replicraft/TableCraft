use super::{Binder, resolve};
use crate::{QueryError, Source, plan::Expr};
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

pub(crate) struct BoundUpdate {
    pub column: usize,
    pub value: Expr,
    pub filter: Option<Expr>,
}

pub(crate) fn bind_update(query: &Query, source: &dyn Source) -> Result<BoundUpdate, QueryError> {
    if !query.parameters.is_empty() {
        return Err(QueryError::Unsupported);
    }
    let Statement::Update(update) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let ([table @ TableSource::Table(_)], [assignment]) =
        (update.sources.as_slice(), update.assignments.as_slice())
    else {
        return Err(QueryError::Unsupported);
    };
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
    let column = resolve(&assignment.target, &relation.slots)?.ok_or(QueryError::UnknownName)?;
    let scopes = [relation.slots.as_slice()];
    let value = binder.expression(&assignment.value, &scopes, false, 1)?;
    let filter = update
        .selection
        .as_ref()
        .map(|filter| binder.expression(filter, &scopes, false, 1))
        .transpose()?;
    if !pure(&value) || filter.as_ref().is_some_and(|filter| !pure(filter)) {
        return Err(QueryError::Unsupported);
    }
    Ok(BoundUpdate {
        column,
        value,
        filter,
    })
}

// Bound recursion is already limited by Binder. Reuse scalar/retrieval semantics,
// but never permit a hidden scan, aggregate, parameter or output-slot dependency.
fn pure(expression: &Expr) -> bool {
    match expression {
        Expr::Literal(_) | Expr::DateLiteral(..) | Expr::Field { depth: 0, .. } => true,
        Expr::Unary(_, value) | Expr::IsNull(value, _) => pure(value),
        Expr::Binary(_, left, right) => pure(left) && pure(right),
        Expr::Between(value, first, last, _) => pure(value) && pure(first) && pure(last),
        Expr::InList(value, values, _) => pure(value) && values.iter().all(pure),
        _ => false,
    }
}
