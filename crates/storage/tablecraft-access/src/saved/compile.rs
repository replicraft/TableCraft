use super::{
    SavedSelect, SavedSelectError,
    context::Context,
    fragments::{self, expression},
    parameters, predicate, relations,
};
use tablecraft_ace::{QueryAttribute as A, SavedQueryDefinition};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{
    ParseLimits,
    query::{ReadBody, Statement},
    select::{Expression, Projection, Select, SortKey},
};

/// Consume inert native fragments into TableCraft-owned local SELECT syntax.
/// Supports zero/one named source or a qualified two-source join, expression
/// projections, an optional appended wildcard, filters, grouping, sorting and
/// SELECT flags and qualified ordered parameter declarations. Larger/compound joins,
/// native external-source fragments, actions and unknown/conflicting metadata
/// fail explicitly. Expression subqueries retain their unresolved syntax.
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
    let fragments = fragments::validate(&definition, &mut context)?;
    let predicate = predicate::read(fragments.flags)?;
    let mut projection = context.reserve(fragments.columns + usize::from(predicate.star))?;
    let mut group_by = context.reserve(fragments.groups)?;
    let mut order_by = context.reserve(fragments.orders)?;
    for row in definition.rows() {
        context.check()?;
        match row.attribute() {
            Some(A::Column) => {
                let text = expression(row)?;
                if text == "*" {
                    if row.name1().is_some() {
                        return Err(SavedSelectError::InvalidMetadata);
                    }
                    context.syntax(text.len())?;
                    projection.push(Projection::Wildcard(None));
                } else {
                    let expression = context.expression(text)?;
                    let alias = row.name1().map(|s| context.name(s)).transpose()?;
                    projection.push(match expression {
                        Expression::Field(field) => Projection::Field { field, alias },
                        expression => Projection::Expression { expression, alias },
                    });
                }
            }
            Some(A::GroupBy) => group_by.push(context.expression(expression(row)?)?),
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
    let from = relations::read(&fragments, &mut context)?;
    let selection = fragments
        .selection
        .map(|row| context.expression(expression(row)?))
        .transpose()?;
    let having = fragments
        .having
        .map(|row| context.expression(expression(row)?))
        .transpose()?;
    let mut select = Select {
        predicate: predicate.syntax,
        projection,
        from,
        group_by,
        having,
        into: None,
        into_external: None,
        external: None,
        owner_access: predicate.owner_access,
        selection,
        order_by,
    };
    let parameters = parameters::read(&definition, fragments.parameters, &mut context)?;
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
