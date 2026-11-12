//! Atomic retained scalar batches bind every original before staging pages.
mod proof;
#[cfg(test)]
mod tests;

use super::{Family, Selected, bind_for, retained_column_shape};
use crate::format::{
    page::PAGE_BYTES,
    row::{OFFSET_MASK, RELOCATED_MASK},
};
use crate::table::mutation::{
    historical_fixed::source::Source,
    record_placement::{
        audit,
        forward::Plan,
        owned::{Reservation, Reservations},
    },
    rows::RowAssignment,
};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

#[derive(Clone, Copy)]
enum Policy {
    Long,
    Currency,
}

impl Policy {
    fn family(self) -> Family {
        match self {
            Self::Long => Family::Retained,
            Self::Currency => Family::RetainedCurrency,
        }
    }

    fn accepts_count(self, count: usize) -> bool {
        match self {
            Self::Long => count >= 2,
            Self::Currency => count != 1,
        }
    }
}

struct Request<'value> {
    selected: Selected,
    value: &'value Value,
}

impl RowAssignment for Request<'_> {
    fn row(&self) -> usize {
        self.selected.ordinal
    }
}

fn bind<'value>(
    image: &NativeImage,
    source: &Source<'_>,
    groups: &'value [FittedColumnUpdates<'value>],
    policy: Policy,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Request<'value>>, EditError> {
    let [group @ FittedColumnUpdates::Fixed { column, updates }] = groups else {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    };
    if !policy.accepts_count(updates.len()) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    // Validate the whole original group, including duplicate target identities,
    // before the old single-cell binder inspects each borrowed request.
    let family = policy.family();
    let position = retained_column_shape(source, group, family, budget, cancelled)?;
    if matches!(policy, Policy::Currency) && updates.is_empty() {
        let schema = source.facts().table.schema();
        let field = &schema.columns()[position];
        for index in source.facts().indexes.indexes() {
            for key in index.columns() {
                check(budget, cancelled)?;
                if key.number() == field.number() {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            }
        }

        for record in &source.facts().records {
            check(budget, cancelled)?;
            family.qualify_record(schema, position, &record.data, budget, cancelled)?;
        }
    }

    budget
        .charge_allocation(updates.len() as u64, size_of::<Request<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    for position in 0..updates.len() {
        check(budget, cancelled)?;
        let single = [FittedColumnUpdates::Fixed {
            column,
            updates: &updates[position..position + 1],
        }];
        let selected = bind_for(image, source, &single, family, budget, cancelled)?;
        let value = updates[position].value();
        result.push(Request { selected, value });
    }
    Ok(result)
}

impl NativeImage {
    pub(crate) fn plan_retained_long_column_batch_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.retained_long_column_batch.plan").entered();
        plan(self, table, groups, Policy::Long, budget, cancelled)
    }

    pub(crate) fn plan_retained_currency_column_batch_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.retained_currency_column_batch.plan").entered();
        plan(self, table, groups, Policy::Currency, budget, cancelled)
    }
}

fn plan(
    image: NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    policy: Policy,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let family = policy.family();
    let source = family.capture(&image, table, budget, cancelled)?;
    let requests = bind(&image, &source, groups, policy, budget, cancelled)?;
    let schema = source.facts().table.schema();
    let captured = family.roots(&image, schema, &requests, budget, cancelled)?;
    let mut records = Vec::new();
    budget
        .charge_allocation(
            captured.selected.len() as u64,
            size_of::<Option<Vec<u8>>>() as u64,
        )
        .map_err(InspectError::from)?;
    records
        .try_reserve_exact(captured.selected.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut changed_rows = 0_usize;
    for root in &captured.selected {
        check(budget, cancelled)?;
        if !matches!(root.word & !OFFSET_MASK, 0 | RELOCATED_MASK) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let original = source
            .facts()
            .records
            .get(root.update.selected.ordinal)
            .ok_or(EditError::Conflict)?;
        if original.offset != root.final_record.offset || original.data != root.final_record.data {
            return Err(EditError::Preservation);
        }
        let record = if root.update.selected.changed {
            changed_rows = changed_rows
                .checked_add(1)
                .ok_or(InspectError::StructureLimit)?;
            Some(family.build_record(
                schema,
                root.update.selected.column,
                &original.data,
                root.update.value,
                budget,
                cancelled,
            )?)
        } else {
            None
        };
        records.push(record);
    }
    let space = family.space(&image, schema, &captured.owned, budget, cancelled)?;
    let mut reservations =
        Reservations::capture(&image, &space, &captured.owned, budget, cancelled)?;
    budget
        .charge_allocation(
            records.len() as u64,
            size_of::<Option<Reservation>>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut reserved = Vec::new();
    reserved
        .try_reserve_exact(records.len())
        .map_err(|_| InspectError::Allocation)?;
    for (root, record) in captured.selected.iter().zip(&records) {
        check(budget, cancelled)?;
        reserved.push(match record {
            Some(record) => {
                Some(reservations.reserve(&image, root, record.len(), budget, cancelled)?)
            }
            None => None,
        });
    }
    // Every original and destination is now bound. One merged page plan is
    // staged in memory; no intermediate database image is published.
    let mut placement = Plan::new();
    for ((root, record), reservation) in captured.selected.iter().zip(&records).zip(&reserved) {
        check(budget, cancelled)?;
        if let (Some(record), Some(reservation)) = (record, reservation) {
            placement.apply_reserved(
                &image,
                root,
                record,
                &space,
                reservation,
                budget,
                cancelled,
            )?;
        }
    }
    audit::pages(&placement, budget, cancelled)?;
    let mut after = family.copy_image(&image, budget, cancelled)?;
    for page in &placement.pages {
        check(budget, cancelled)?;
        let at = usize::try_from(page.number)
            .ok()
            .and_then(|number| number.checked_mul(PAGE_BYTES))
            .ok_or(EditError::Preservation)?;
        let end = at.checked_add(PAGE_BYTES).ok_or(EditError::Preservation)?;
        let target = after.get_mut(at..end).ok_or(EditError::Preservation)?;
        if target != page.before.as_slice() || page.after.len() != PAGE_BYTES {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        target.copy_from_slice(&page.after);
    }
    audit::final_pages(&placement, &after, budget, cancelled)?;
    space.audit(image.as_bytes(), &after, budget, cancelled)?;
    proof::prove(&image, table, groups, &after, policy, budget, cancelled)?;
    drop(source);
    Ok(NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows,
    })
}
