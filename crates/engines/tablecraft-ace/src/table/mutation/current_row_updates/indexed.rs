//! Retained manual LONG rows and INDEX populations change in one native action.
mod proof;
mod source;
#[cfg(test)]
mod tests;

use super::Family;
use crate::table::{
    indexes::{IndexKeyUpdate, MutationKey, composite::DecimalValue, mutation::normal_fitting},
    mutation::{
        fixed_records::{self, indexed::i32_updates::I32UpdateIndexes},
        historical_fixed::source::Source,
        normal_fitted::{binding, original_values},
        preserved_source::validation,
    },
};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

struct Request<'value> {
    row: usize,
    column: usize,
    value: &'value Value,
    changed: bool,
}

fn bind<'value>(
    image: &NativeImage,
    source: &Source<'_>,
    groups: &[FittedColumnUpdates<'value>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Request<'value>>, EditError> {
    let [
        FittedColumnUpdates::Fixed {
            column: name,
            updates,
        },
    ] = groups
    else {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    };
    let updates = *updates;
    let column = source::column(source, name, budget, cancelled)?;
    let shaped =
        super::retained_column_shape(source, &groups[0], Family::Retained, budget, cancelled)?;
    if column != shaped {
        return Err(EditError::Preservation);
    }
    // Qualify every original selected field, including zero-match/no-op actions.
    for record in &source.facts().records {
        Family::Retained.qualify_record(
            source.facts().table.schema(),
            column,
            &record.data,
            budget,
            cancelled,
        )?;
    }
    budget
        .charge_allocation(updates.len() as u64, size_of::<Request<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    for update in updates {
        check(budget, cancelled)?;
        if !matches!(update.expected(), Value::Int32(_) | Value::Null)
            || !matches!(update.value(), Value::Int32(_) | Value::Null)
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let values = original_values(
            image,
            source.facts(),
            update.row(),
            &[column],
            budget,
            cancelled,
        )?;
        let original = values.first().ok_or(EditError::Preservation)?;
        if !binding::equal(original, update.expected(), budget, cancelled)? {
            return Err(EditError::Conflict);
        }
        let changed = !binding::equal(original, update.value(), budget, cancelled)?;
        result.push(Request {
            row: update.row(),
            column,
            value: update.value(),
            changed,
        });
    }
    Ok(result)
}

impl NativeImage {
    pub(crate) fn plan_retained_indexed_long_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.retained_indexed_long.plan").entered();
        let source = source::capture(&self, table, budget, cancelled)?;
        let requests = bind(&self, &source, groups, budget, cancelled)?;
        let facts = I32UpdateIndexes::capture(&self, table, source.facts(), budget, cancelled)?;
        let mut records = Vec::new();
        budget
            .charge_allocation(requests.len() as u64, size_of::<Option<Vec<u8>>>() as u64)
            .map_err(InspectError::from)?;
        records
            .try_reserve_exact(requests.len())
            .map_err(|_| InspectError::Allocation)?;
        let mut updates = Vec::new();
        budget
            .charge_allocation(requests.len() as u64, size_of::<IndexKeyUpdate>() as u64)
            .map_err(InspectError::from)?;
        updates
            .try_reserve_exact(requests.len())
            .map_err(|_| InspectError::Allocation)?;
        let kind = MutationKey::new(ScalarType::Int32)?;
        let mut changed_rows = 0_usize;
        for request in &requests {
            check(budget, cancelled)?;
            if !request.changed {
                records.push(None);
                continue;
            }
            let record = source
                .facts()
                .records
                .get(request.row)
                .ok_or(EditError::Conflict)?;
            let encoded = Family::Retained.build_record(
                source.facts().table.schema(),
                request.column,
                &record.data,
                request.value,
                budget,
                cancelled,
            )?;
            if encoded.len() != record.data.len() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let original = original_values(
                &self,
                source.facts(),
                request.row,
                &[request.column],
                budget,
                cancelled,
            )?;
            updates.push(IndexKeyUpdate {
                reference: record.reference,
                column: request.column,
                expected: stored(original.first().ok_or(EditError::Preservation)?, &kind)?,
                value: stored(request.value, &kind)?,
            });
            changed_rows = changed_rows
                .checked_add(1)
                .ok_or(InspectError::StructureLimit)?;
            records.push(Some(encoded));
        }
        // All final keys are submitted together, including simultaneous swaps.
        // INDEX and DATA staging remain private until independent full-image proof.
        let indexes = normal_fitting::patches(&self, facts.checked(), &updates, budget, cancelled)?;
        let mut after = fixed_records::copy(self.as_bytes(), budget, cancelled)?;
        for (request, bytes) in requests.iter().zip(&records) {
            if let Some(bytes) = bytes {
                let record = &source.facts().records[request.row];
                apply(
                    &mut after,
                    record.offset,
                    &record.data,
                    bytes,
                    budget,
                    cancelled,
                )?;
            }
        }
        for patch in indexes {
            apply(
                &mut after,
                patch.offset,
                &patch.before,
                &patch.after,
                budget,
                cancelled,
            )?;
        }
        validation::expected(
            &self,
            table,
            &source.facts().table,
            &after,
            budget,
            cancelled,
        )?;
        proof::prove(
            &self,
            table,
            groups,
            &after,
            changed_rows,
            budget,
            cancelled,
        )?;
        drop(facts);
        drop(source);
        Ok(NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows,
        })
    }
}

fn stored(value: &Value, kind: &MutationKey) -> Result<Option<DecimalValue>, EditError> {
    match value {
        Value::Null => Ok(None),
        Value::Int32(value) => Ok(Some(kind.stored(&value.to_le_bytes())?)),
        _ => Err(EditError::Unsupported(WriteBlocker::Column)),
    }
}

fn apply(
    after: &mut [u8],
    offset: usize,
    before: &[u8],
    replacement: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if before.len() != replacement.len() {
        return Err(EditError::Preservation);
    }
    let end = offset
        .checked_add(before.len())
        .ok_or(EditError::Preservation)?;
    let output = after.get_mut(offset..end).ok_or(EditError::Preservation)?;
    budget
        .charge(Resource::Steps, before.len() as u64)
        .map_err(InspectError::from)?;
    if output != before {
        return Err(EditError::Preservation);
    }
    output.copy_from_slice(replacement);
    Ok(())
}
