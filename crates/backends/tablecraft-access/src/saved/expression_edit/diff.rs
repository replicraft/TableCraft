//! Exact complete-model guards and source-derived existing fragment selection.
use super::super::{SavedRead, context::Context, fragments, ordering};
use super::{SavedQuerySqlEditError as Error, render};
use tablecraft_ace::{QueryAttribute, QueryFragmentExpressionEdit, SavedQueryRow};
use tablecraft_sql::{
    query::{Query, ReadBody, ReadQuery, Statement},
    select::{Name, Projection, Select, SelectPredicate, TableSource},
};

pub(super) struct Change<'a> {
    row: &'a SavedQueryRow,
    replacement: String,
}

impl Change<'_> {
    pub fn request(&self) -> Result<QueryFragmentExpressionEdit<'_>, Error> {
        Ok(QueryFragmentExpressionEdit {
            attribute: self.row.attribute().ok_or(Error::Unsupported)?,
            order: self
                .row
                .order()
                .and_then(|order| order.try_into().ok())
                .ok_or(Error::Unsupported)?,
            expected: self.row.expression().ok_or(Error::Unsupported)?,
            replacement: &self.replacement,
        })
    }
}

pub(super) fn changes<'a>(
    actual: &'a SavedRead,
    proposed: &Query,
    context: &mut Context<'_>,
) -> Result<Vec<Change<'a>>, Error> {
    let (before_read, before) = select(actual.query())?;
    let (after_read, after) = select(proposed)?;
    guard(
        actual.query(),
        proposed,
        before_read,
        after_read,
        before,
        after,
    )?;
    let rows = ordering::read(actual.definition(), context)?;
    let fragments = fragments::validate(actual.definition(), &rows, context)?;
    let mut columns = context.reserve::<&SavedQueryRow>(fragments.columns)?;
    for &row in &rows {
        context.check()?;
        if row.attribute() == Some(QueryAttribute::Column) && fragments.is_row_column(row) {
            columns.push(row);
        }
    }
    if columns.len() != before.projection.len() {
        return Err(Error::Unsupported);
    }

    let mut changes =
        context.reserve(before.projection.len() + usize::from(before.selection.is_some()))?;
    for ((original, proposed), row) in before.projection.iter().zip(&after.projection).zip(columns)
    {
        context.check()?;
        if alias(original) != alias(proposed) {
            return Err(Error::Unsupported);
        }
        if !render::same_projection_body(original, proposed) {
            render::validate_projection(original, context)?;
            changes.push(Change {
                row,
                replacement: render::projection(proposed, context)?,
            });
        }
    }
    match (&before.selection, &after.selection) {
        (Some(original), Some(proposed)) if original != proposed => {
            render::validate_expression(original, context)?;
            changes.push(Change {
                row: fragments.selection.ok_or(Error::Unsupported)?,
                replacement: render::expression(proposed, context)?,
            });
        }
        (Some(_), Some(_)) | (None, None) => {}
        _ => return Err(Error::Unsupported),
    }
    Ok(changes)
}

fn select(query: &Query) -> Result<(&ReadQuery, &Select), Error> {
    let Statement::Read(read) = &query.statement else {
        return Err(Error::Unsupported);
    };
    let ReadBody::Select(select) = &read.body else {
        return Err(Error::Unsupported);
    };
    Ok((read, select))
}

fn guard(
    original: &Query,
    proposed: &Query,
    before_read: &ReadQuery,
    after_read: &ReadQuery,
    before: &Select,
    after: &Select,
) -> Result<(), Error> {
    if original.parameters != proposed.parameters
        || !original.parameters.is_empty()
        || before_read.order_by != after_read.order_by
        || before_read.owner_access != after_read.owner_access
        || before.predicate != after.predicate
        || before.projection.len() != after.projection.len()
        || before.from != after.from
        || before.group_by != after.group_by
        || before.having != after.having
        || before.into != after.into
        || before.into_external != after.into_external
        || before.external != after.external
        || before.owner_access != after.owner_access
        || before.order_by != after.order_by
        || before.selection.is_some() != after.selection.is_some()
    {
        return Err(Error::Unsupported);
    }
    // This adapter slice mirrors the current native local SELECT topology. It
    // does not broaden the engine by formatting an unsupported source clause.
    if before.predicate != SelectPredicate::All
        || !before.group_by.is_empty()
        || before.having.is_some()
        || before.into.is_some()
        || before.into_external.is_some()
        || before.external.is_some()
        || before.owner_access
        || before_read.owner_access
        || !before.order_by.is_empty()
        || !before_read.order_by.is_empty()
    {
        return Err(Error::Unsupported);
    }
    for source in &before.from {
        if !matches!(source, TableSource::Table(table) if table.database.is_none()) {
            return Err(Error::Unsupported);
        }
    }
    Ok(())
}

fn alias(projection: &Projection) -> Option<&Name> {
    match projection {
        Projection::Expression { alias, .. } | Projection::Field { alias, .. } => alias.as_ref(),
        Projection::Wildcard(_) => None,
    }
}
