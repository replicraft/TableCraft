use super::{
    SavedSelectError,
    context::Context,
    fragments::{Fragments, expression},
};
use std::borrow::Cow;

const PARTS_PER_CONDITION: usize = 4;

pub(super) fn read<'a>(
    fragments: &Fragments<'a>,
    context: &mut Context<'_>,
) -> Result<Cow<'a, str>, SavedSelectError> {
    let first = fragments
        .joins
        .first()
        .copied()
        .flatten()
        .ok_or(SavedSelectError::InvalidMetadata)?;
    let original = expression(first)?;
    if fragments.join_count == 1 {
        return Ok(Cow::Borrowed(original));
    }
    let left = first.name1().ok_or(SavedSelectError::InvalidMetadata)?;
    let right = first.name2().ok_or(SavedSelectError::InvalidMetadata)?;
    let mut parts = context.reserve(fragments.join_count * PARTS_PER_CONDITION)?;
    // QueryImpl's same-pair addCondition prepends each subsequent stored
    // condition. Preserve that order rather than flattening or sorting it.
    for row in fragments.joins.iter().take(fragments.join_count).rev() {
        context.check()?;
        let row = row.ok_or(SavedSelectError::InvalidMetadata)?;
        let row_left = row.name1().ok_or(SavedSelectError::InvalidMetadata)?;
        let row_right = row.name2().ok_or(SavedSelectError::InvalidMetadata)?;
        if !row_left.is_ascii() || !row_right.is_ascii() {
            return Err(SavedSelectError::Unsupported);
        }
        if row.flag().is_none() {
            return Err(SavedSelectError::InvalidMetadata);
        }
        if row.flag() != first.flag()
            || !row_left.eq_ignore_ascii_case(left)
            || !row_right.eq_ignore_ascii_case(right)
        {
            return Err(SavedSelectError::Unsupported);
        }
        let condition = expression(row)?;
        // Each stored expression must be valid independently; combining raw
        // strings must not repair parentheses or introduce additional clauses.
        context.expression(condition)?;
        if !parts.is_empty() {
            parts.push(" AND ");
        }
        parts.extend(["(", condition, "\n)"]);
    }
    Ok(Cow::Owned(context.text(&parts)?))
}
