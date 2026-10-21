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
        budget: None,
        cancelled: None,
        source,
        parameters: Vec::new(),
        parameter_scope: Vec::new(),
        retrieval: false,
        active_queries: Vec::new(),
        nodes: 0,
        policy_visits: 0,
        bytes: 0,
    };
    let relation = binder.relation(table, &[], 1)?;
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
        Expr::Literal(_) | Expr::Field { depth: 0, .. } => true,
        Expr::Unary(_, value) | Expr::IsNull(value, _) => pure(value),
        Expr::Binary(_, left, right) => pure(left) && pure(right),
        Expr::Between(value, first, last, _) => pure(value) && pure(first) && pure(last),
        Expr::InList(value, values, _) => pure(value) && values.iter().all(pure),
        _ => false,
    }
}
