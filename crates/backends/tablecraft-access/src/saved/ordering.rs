//! Borrowed, stable native fragment ordering without mutating the definition.
use super::{SavedReadError as E, context::Context};
use std::cmp::Ordering;
use tablecraft_ace::{SavedQueryDefinition, SavedQueryRow};
use tablecraft_core::limits::Resource;

const NULL_ATTRIBUTE: i16 = -1;

pub(super) fn read<'a>(
    definition: &'a SavedQueryDefinition,
    context: &mut Context<'_>,
) -> Result<Vec<&'a SavedQueryRow>, E> {
    let mut rows = context.reserve(definition.rows().len())?;
    let mut complete = true;
    for row in definition.rows() {
        context.check()?;
        complete &= row.order().is_some();
        rows.push(row);
    }
    // The reference preserves all physical order if any fragment lacks a key,
    // rather than partially sorting only the clauses whose keys are present.
    if !complete || rows.len() < 2 {
        return Ok(rows);
    }
    let mut sorted = true;
    for pair in rows.windows(2) {
        if compare(pair[0], pair[1], context)?.is_gt() {
            sorted = false;
            break;
        }
    }
    if sorted {
        return Ok(rows);
    }
    let mut scratch = context.reserve(rows.len())?;
    context.budget.charge(Resource::Steps, rows.len() as u64)?;
    scratch.extend_from_slice(&rows);
    // Stable bottom-up merging has bounded O(n log n) comparisons, checks work
    // and cancellation during every merge, and uses one charged scratch vector.
    let mut width = 1;
    while width < rows.len() {
        context.check()?;
        let stride = width.saturating_mul(2);
        for start in (0..rows.len()).step_by(stride) {
            let middle = start.saturating_add(width).min(rows.len());
            let end = start.saturating_add(stride).min(rows.len());
            let (mut left, mut right) = (start, middle);
            for slot in &mut scratch[start..end] {
                context.check()?;
                if left < middle
                    && (right == end || !compare(rows[left], rows[right], context)?.is_gt())
                {
                    *slot = rows[left];
                    left += 1;
                } else {
                    *slot = rows[right];
                    right += 1;
                }
            }
        }
        std::mem::swap(&mut rows, &mut scratch);
        width = stride;
    }
    context.check()?;
    Ok(rows)
}
fn compare(
    left: &SavedQueryRow,
    right: &SavedQueryRow,
    context: &mut Context<'_>,
) -> Result<Ordering, E> {
    context.check()?;
    // Native attributes are signed bytes in the reference; unknown/NULL tags
    // still reach normal metadata validation and cannot acquire SQL semantics.
    let attribute = |row: &SavedQueryRow| {
        row.attribute()
            .map_or(NULL_ATTRIBUTE, |tag| i16::from(tag.code() as i8))
    };
    let order = attribute(left).cmp(&attribute(right));
    if !order.is_eq() {
        return Ok(order);
    }
    let (Some(left), Some(right)) = (left.order(), right.order()) else {
        return Err(E::InvalidMetadata);
    };
    context
        .budget
        .charge(Resource::Steps, left.len().min(right.len()) as u64 + 1)?;
    Ok(left.cmp(right))
}
