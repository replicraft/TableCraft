//! Native crosstab roles and adaptation to owned TRANSFORM syntax.
use super::{
    SavedRead, SavedReadError as E, compile,
    context::Context,
    fragments::{self, Profile, expression},
    ordering, parameters,
};
use tablecraft_ace::{SavedQueryDefinition, SavedQueryRow};
use tablecraft_sql::{
    query::{Crosstab, Query},
    select::Expression,
};

pub(super) const FAMILY: u32 = 0x10;
pub(super) const TYPE: i16 = 6;
pub(super) const ROW_FLAG: i16 = 0x02;
const PIVOT_FLAG: i16 = 0x01;
const TRANSFORM_FLAG: i16 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Row,
    Transform,
    Pivot,
}
pub(super) fn column_role(row: &SavedQueryRow) -> Result<Role, E> {
    match row.flag().unwrap_or(TRANSFORM_FLAG) {
        ROW_FLAG => Ok(Role::Row),
        TRANSFORM_FLAG => Ok(Role::Transform),
        PIVOT_FLAG if row.name1().is_none() => Ok(Role::Pivot),
        _ => Err(E::Unsupported),
    }
}
pub(super) fn group_role(row: &SavedQueryRow) -> Result<Role, E> {
    match row.flag() {
        Some(ROW_FLAG) => Ok(Role::Row),
        Some(PIVOT_FLAG) => Ok(Role::Pivot),
        _ => Err(E::Unsupported),
    }
}
pub(super) fn reconstruct(
    definition: SavedQueryDefinition,
    context: &mut Context<'_>,
) -> Result<SavedRead, E> {
    context.check()?;
    let rows = ordering::read(&definition, context)?;
    let fragments = fragments::validate_profile(&definition, &rows, context, Profile::Crosstab)?;
    let aggregate = fragments.transform.ok_or(E::InvalidMetadata)?;
    let transform = context.expression(expression(aggregate)?)?;
    let transform_alias = aggregate
        .name1()
        .map(|name| context.name(name))
        .transpose()?;
    let pivot_row = fragments.pivot.ok_or(E::InvalidMetadata)?;
    let pivot = context.expression(expression(pivot_row)?)?;
    let (pivot, headings) = match pivot {
        Expression::InList {
            expression,
            values,
            negated: false,
        } => (*expression, Some(values)),
        pivot => (pivot, None),
    };
    // This redundant native grouping row describes pivot membership, not a row
    // grouping key. Accept it only when its owned expression agrees exactly.
    if let Some(group) = fragments.pivot_group {
        let expression = context.expression(expression(group)?)?;
        if expression != pivot {
            return Err(E::InvalidMetadata);
        }
    }
    let select = compile::select_body(&rows, &fragments, context)?;
    let parameters = parameters::read(&rows, fragments.parameters, context)?;
    context.construction("TRANSFORM Count(*) SELECT 0 GROUP BY 0 PIVOT 0".len(), 12)?;
    let mut query = Query::from_crosstab(Crosstab {
        transform,
        transform_alias,
        select,
        pivot,
        headings,
    });
    query.parameters = parameters;
    context.check()?;
    Ok(SavedRead { definition, query })
}
