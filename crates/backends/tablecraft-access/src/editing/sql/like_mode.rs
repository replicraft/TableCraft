//! Capture criteria wildcard syntax from the same immutable mutation source.
use tablecraft_ace::{DatabaseQueryMode, InspectError, NativeImage, inspect_database_properties};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    query::{Query, ReadBody, ReadQuery, Statement},
    select::{Expression, FunctionArgument, TableSource},
};

enum Visit<'a> {
    Expression(&'a Expression),
    Relation(&'a TableSource),
    Read(&'a ReadQuery),
}

/// Observe raw native query-mode metadata only for written criteria LIKE.
/// The image must be the immutable source subsequently moved into the editor.
/// Assignments and projected expressions retain their separate closed admission.
/// None means no criteria LIKE. Some retains Absent separately from Unknown;
/// the caller owns any explicit application default. No default is invented here.
///
/// # Errors
/// Returns native metadata errors, cancellation, allocation failure or exhaustion
/// of either the syntax-work or native-inspection allowance. No file is opened.
pub fn native_mutation_query_mode(
    image: &NativeImage,
    query: &Query,
    native: &mut ResourceBudget,
    syntax: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<DatabaseQueryMode>, InspectError> {
    let _span = tracing::debug_span!("access.mutation.like_mode").entered();
    if !uses_like(query, syntax, cancelled)? {
        return Ok(None);
    }
    let properties = inspect_database_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        native,
        cancelled,
    )?;
    let mode = match properties {
        Some(properties) => properties.query_mode(native, cancelled)?,
        None => DatabaseQueryMode::Absent,
    };
    Ok(Some(mode))
}

fn uses_like(
    query: &Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    let (sources, filter) = match &query.statement {
        Statement::Update(update) => (&update.sources, update.selection.as_ref()),
        Statement::Delete(delete) => (&delete.sources, delete.selection.as_ref()),
        _ => return Ok(false),
    };
    let mut pending = Vec::new();
    for source in sources {
        push(&mut pending, Visit::Relation(source), budget, cancelled)?;
    }
    if let Some(filter) = filter {
        push(&mut pending, Visit::Expression(filter), budget, cancelled)?;
    }
    while let Some(visit) = pending.pop() {
        check(budget, cancelled)?;
        match visit {
            Visit::Expression(expression) => match expression {
                Expression::Like { .. } => return Ok(true),
                Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
                    push(
                        &mut pending,
                        Visit::Expression(expression),
                        budget,
                        cancelled,
                    )?;
                }
                Expression::Binary { left, right, .. } => {
                    push(&mut pending, Visit::Expression(left), budget, cancelled)?;
                    push(&mut pending, Visit::Expression(right), budget, cancelled)?;
                }
                Expression::Between {
                    expression,
                    first_bound,
                    second_bound,
                    ..
                } => {
                    for value in [
                        expression.as_ref(),
                        first_bound.as_ref(),
                        second_bound.as_ref(),
                    ] {
                        push(&mut pending, Visit::Expression(value), budget, cancelled)?;
                    }
                }
                Expression::InList {
                    expression, values, ..
                } => {
                    push(
                        &mut pending,
                        Visit::Expression(expression),
                        budget,
                        cancelled,
                    )?;
                    for value in values {
                        push(&mut pending, Visit::Expression(value), budget, cancelled)?;
                    }
                }
                Expression::Call { arguments, .. } => {
                    for argument in arguments {
                        check(budget, cancelled)?;
                        if let FunctionArgument::Expression(value) = argument {
                            push(&mut pending, Visit::Expression(value), budget, cancelled)?;
                        }
                    }
                }
                Expression::Exists(read) => {
                    push(&mut pending, Visit::Read(read), budget, cancelled)?
                }
                Expression::InQuery {
                    expression, query, ..
                } => {
                    push(
                        &mut pending,
                        Visit::Expression(expression),
                        budget,
                        cancelled,
                    )?;
                    push(&mut pending, Visit::Read(query), budget, cancelled)?;
                }
                _ => {}
            },
            Visit::Relation(source) => match source {
                TableSource::Join {
                    left,
                    right,
                    condition,
                    ..
                } => {
                    push(
                        &mut pending,
                        Visit::Expression(condition),
                        budget,
                        cancelled,
                    )?;
                    push(&mut pending, Visit::Relation(left), budget, cancelled)?;
                    push(&mut pending, Visit::Relation(right), budget, cancelled)?;
                }
                TableSource::Group { source, .. } => {
                    push(&mut pending, Visit::Relation(source), budget, cancelled)?;
                }
                // Derived/named sources remain outside mutation admission.
                _ => {}
            },
            Visit::Read(read) => {
                if let ReadBody::Select(select) = &read.body {
                    if let Some(filter) = &select.selection {
                        push(&mut pending, Visit::Expression(filter), budget, cancelled)?;
                    }
                    for source in &select.from {
                        push(&mut pending, Visit::Relation(source), budget, cancelled)?;
                    }
                }
            }
        }
    }
    Ok(false)
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), InspectError> {
    if cancelled() {
        return Err(InspectError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

fn push<'a>(
    pending: &mut Vec<Visit<'a>>,
    value: Visit<'a>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    if pending.len() == pending.capacity() {
        let added = pending.capacity().max(1);
        budget.charge(Resource::Steps, pending.len() as u64)?;
        budget.charge_allocation(added as u64, size_of::<Visit<'_>>() as u64)?;
        pending
            .try_reserve_exact(added)
            .map_err(|_| InspectError::Allocation)?;
    }
    pending.push(value);
    Ok(())
}

#[cfg(test)]
mod tests;
