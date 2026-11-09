//! Explicit native expression placement; no ordinary system-row UPDATE admission.
mod binding;
mod record;
mod source;

use super::{
    EditError,
    record_placement::{audit, forward::Plan, space::Space},
    record_roots,
};
use crate::{
    InspectError, NativeImage, NativeObjectOrigin, QueryExpressionEditsError,
    QueryFragmentExpressionEdit, header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(crate) fn build(
    image: &NativeImage,
    origin: &NativeObjectOrigin,
    edits: &[QueryFragmentExpressionEdit<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, QueryExpressionEditsError> {
    let schema = source::qualify(image, origin, budget, cancelled)?;
    budget.charge_allocation(
        edits.len() as u64,
        size_of::<binding::Expected<'_>>() as u64,
    )?;
    let mut requests = Vec::new();
    requests
        .try_reserve_exact(edits.len())
        .map_err(|_| InspectError::Allocation)?;
    for edit in edits {
        check(budget, cancelled)?;
        requests.push(binding::Expected {
            native_id: origin.native_id(),
            attribute: edit.attribute,
            order: edit.order,
            expression: edit.expected,
        });
    }
    let selected = binding::capture(image, &schema, &requests, budget, cancelled).map_err(error)?;
    let roots =
        record_roots::capture(image, &schema, &selected, budget, cancelled).map_err(error)?;
    let space =
        Space::capture_normal(image, &schema, &roots.owned, budget, cancelled).map_err(error)?;
    let mut placement = Plan::new();
    // The submitted order is part of this new plan purpose. Original key/order
    // and source ordinals are independent; do not silently sort physical writes.
    for (request, edit) in edits.iter().enumerate() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, roots.selected.len() as u64)?;
        let root = roots
            .selected
            .iter()
            .find(|root| root.update.request == request)
            .ok_or(QueryExpressionEditsError::Preservation)?;
        if root.reference != root.update.reference {
            return Err(QueryExpressionEditsError::Preservation);
        }
        let unchanged = record::replace(
            &schema,
            &root.final_record.data,
            edit.expected,
            budget,
            cancelled,
        )
        .map_err(error)?;
        if unchanged != root.final_record.data {
            return Err(QueryExpressionEditsError::Preservation);
        }
        let record = record::replace(
            &schema,
            &root.final_record.data,
            edit.replacement,
            budget,
            cancelled,
        )
        .map_err(error)?;
        placement
            .apply(image, root, &record, &space, budget, cancelled)
            .map_err(error)?;
    }
    audit::pages(&placement, budget, cancelled).map_err(error)?;
    let length = image.as_bytes().len();
    budget.charge_allocation(length as u64, 1)?;
    budget.charge(Resource::Steps, length as u64)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    after.extend_from_slice(image.as_bytes());
    for page in &placement.pages {
        check(budget, cancelled)?;
        let offset = page.number as usize * crate::format::page::PAGE_BYTES;
        after
            .get_mut(offset..offset + page.after.len())
            .ok_or(QueryExpressionEditsError::Preservation)?
            .copy_from_slice(&page.after);
    }
    audit::final_pages(&placement, &after, budget, cancelled).map_err(error)?;
    // Original protected map/definition/index/LVAL storage remains literal.
    space
        .audit(image.as_bytes(), &after, budget, cancelled)
        .map_err(error)?;
    check(budget, cancelled)?;
    Ok(after)
}

fn error(error: EditError) -> QueryExpressionEditsError {
    match error {
        EditError::Inspect(InspectError::Unsupported) => QueryExpressionEditsError::Source,
        EditError::Inspect(error) => QueryExpressionEditsError::Inspect(error),
        EditError::Preservation => QueryExpressionEditsError::Preservation,
        _ => QueryExpressionEditsError::Source,
    }
}

pub(crate) fn verify(
    before: &NativeImage,
    after: &NativeImage,
    origin: &NativeObjectOrigin,
    fresh_origin: &NativeObjectOrigin,
    edits: &[QueryFragmentExpressionEdit<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryExpressionEditsError> {
    let old_schema = source::qualify(before, origin, budget, cancelled)?;
    let new_schema = source::qualify(after, fresh_origin, budget, cancelled)?;
    budget.charge_allocation(
        (2 * edits.len()) as u64,
        size_of::<binding::Expected<'_>>() as u64,
    )?;
    let mut old = Vec::new();
    let mut new = Vec::new();
    old.try_reserve_exact(edits.len())
        .map_err(|_| InspectError::Allocation)?;
    new.try_reserve_exact(edits.len())
        .map_err(|_| InspectError::Allocation)?;
    for edit in edits {
        check(budget, cancelled)?;
        old.push(binding::Expected {
            native_id: origin.native_id(),
            attribute: edit.attribute,
            order: edit.order,
            expression: edit.expected,
        });
        new.push(binding::Expected {
            native_id: fresh_origin.native_id(),
            attribute: edit.attribute,
            order: edit.order,
            expression: edit.replacement,
        });
    }
    let old = binding::capture(before, &old_schema, &old, budget, cancelled).map_err(error)?;
    let new = binding::capture(after, &new_schema, &new, budget, cancelled).map_err(error)?;
    if old.len() != new.len() || old_schema.declared_rows() != new_schema.declared_rows() {
        return Err(QueryExpressionEditsError::Preservation);
    }
    for (old, new) in old.iter().zip(&new) {
        check(budget, cancelled)?;
        if old.request != new.request || old.reference != new.reference {
            return Err(QueryExpressionEditsError::Preservation);
        }
    }
    let roots =
        record_roots::capture(after, &new_schema, &new, budget, cancelled).map_err(error)?;
    Space::capture_normal(after, &new_schema, &roots.owned, budget, cancelled).map_err(error)?;
    Ok(())
}
