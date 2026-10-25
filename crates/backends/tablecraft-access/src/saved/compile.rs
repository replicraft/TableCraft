use super::{
    SavedSelect, SavedSelectError,
    context::Context,
    external,
    fragments::{self, expression},
    ordering, parameters, predicate, relations,
};
use tablecraft_ace::{QueryAttribute as A, SavedQueryDefinition, SavedQueryRow};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{
    ParseLimits,
    query::{ReadBody, Statement},
    select::{Projection, Select, SortKey},
};

/// Consume inert native fragments into TableCraft-owned local SELECT syntax.
/// Supports bounded named/derived-source join forests and comma products, expression
/// projections, an optional appended wildcard, filters, grouping, sorting and
/// SELECT flags and qualified ordered parameter declarations. Excessive join graphs,
/// actions and unknown/conflicting metadata fail explicitly. Global and per-table
/// external selectors remain inert owned metadata. Expression subqueries retain
/// their unresolved syntax.
///
/// This does not bind sources/functions, open providers or execute content. The
/// native source must have been stable/offline while reading the definition.
pub fn reconstruct_saved_select(
    definition: SavedQueryDefinition,
    limits: ParseLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SavedSelect, SavedSelectError> {
    let _span = tracing::debug_span!("access.query.reconstruct").entered();
    let mut context = Context::new(budget, cancelled, limits);
    context.check()?;
    let rows = ordering::read(&definition, &mut context)?;
    let fragments = fragments::validate(&definition, &rows, &mut context)?;
    let mut select = select_body(&rows, &fragments, &mut context)?;
    let parameters = parameters::read(&rows, fragments.parameters, &mut context)?;
    // The bounded parser supplies the owned boxed statement carrier. Move the
    // compiled SELECT into it; do not add unchecked Box allocation or AST clones.
    let mut query = context.query(&["SELECT 0"])?;
    let Statement::Read(read) = &mut query.statement else {
        return Err(SavedSelectError::InvalidMetadata);
    };
    let ReadBody::Select(body) = &mut read.body else {
        return Err(SavedSelectError::InvalidMetadata);
    };
    read.order_by = std::mem::take(&mut select.order_by);
    read.owner_access = std::mem::replace(&mut select.owner_access, false);
    **body = select;
    query.parameters = parameters;
    context.check()?;
    Ok(SavedSelect { definition, query })
}

/// Compose validated SELECT clauses shared by SELECT and crosstab families.
pub(super) fn select_body(
    rows: &[&SavedQueryRow],
    fragments: &fragments::Fragments<'_>,
    context: &mut Context<'_>,
) -> Result<Select, SavedSelectError> {
    let predicate = predicate::read(fragments.flags)?;
    let mut projection = context.reserve(fragments.columns + usize::from(predicate.star))?;
    let mut group_by = context.reserve(fragments.groups)?;
    let mut order_by = context.reserve(fragments.orders)?;
    for &row in rows {
        context.check()?;
        match row.attribute() {
            Some(A::Column) if fragments.is_row_column(row) => {
                let text = expression(row)?;
                let parsed = context.projection(text)?;
                if matches!(parsed, Projection::Wildcard(_)) && row.name1().is_some() {
                    return Err(SavedSelectError::InvalidMetadata);
                }
                let alias = row.name1().map(|name| context.name(name)).transpose()?;
                projection.push(match parsed {
                    Projection::Wildcard(qualifier) => Projection::Wildcard(qualifier),
                    Projection::Field { field, .. } => Projection::Field { field, alias },
                    Projection::Expression { expression, .. } => {
                        Projection::Expression { expression, alias }
                    }
                });
            }
            Some(A::GroupBy) if fragments.is_row_group(row) => {
                group_by.push(context.expression(expression(row)?)?)
            }
            Some(A::OrderBy) => order_by.push(SortKey {
                expression: context.expression(expression(row)?)?,
                descending: row.name1().is_some_and(|s| s.eq_ignore_ascii_case("D")),
            }),
            _ => {}
        }
    }
    if predicate.star {
        context.syntax("*".len())?;
        projection.push(Projection::Wildcard(None));
    }
    if projection.is_empty() {
        return Err(SavedSelectError::InvalidMetadata);
    }
    let from = relations::read(fragments, context)?;
    let external = external::read(fragments.external, !from.is_empty(), context)?;
    let selection = fragments
        .selection
        .map(|row| context.expression(expression(row)?))
        .transpose()?;
    let having = fragments
        .having
        .map(|row| context.expression(expression(row)?))
        .transpose()?;
    let select = Select {
        predicate: predicate.syntax,
        projection,
        from,
        group_by,
        having,
        into: None,
        into_external: None,
        external,
        owner_access: predicate.owner_access,
        selection,
        order_by,
    };
    Ok(select)
}
