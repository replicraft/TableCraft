//! Maintain each physical owner once, in order, through the shared EOF cursor.
use super::{RowInsert, placement::InsertedRow};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    header::check,
    table::{
        indexes::mutation::insertion::{self, IndexInsertion, InsertKey},
        mutation::append::State,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn prepare(
    image: &NativeImage,
    checked: &TableCheck,
    requests: &[RowInsert],
    rows: &[InsertedRow],
    mut append: State,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<IndexInsertion>, State), EditError> {
    check(budget, cancelled)?;
    if rows.len() != requests.len() {
        return Err(EditError::Preservation);
    }
    let count = checked.schema().layout.physical_indexes;
    if count == 0 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    budget
        .charge_allocation(count as u64, size_of::<IndexInsertion>() as u64)
        .map_err(InspectError::from)?;
    let mut plans: Vec<IndexInsertion> = Vec::new();
    plans
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;

    for number in 0..count {
        check(budget, cancelled)?;
        let physical = u32::try_from(number).map_err(|_| InspectError::StructureLimit)?;
        let mut selected = None;
        for entry in checked.indexes() {
            check(budget, cancelled)?;
            if entry.index().data_number() == physical {
                selected = Some(entry.index());
                break;
            }
        }
        let index = selected.ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        let [component] = index.columns() else {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        };
        let mut column = None;
        for (position, field) in checked.schema().columns().iter().enumerate() {
            check(budget, cancelled)?;
            if field.number() == component.number() {
                column = Some(position);
                break;
            }
        }
        let column = column.ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        budget
            .charge_allocation(requests.len() as u64, size_of::<InsertKey<'_>>() as u64)
            .map_err(InspectError::from)?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(requests.len())
            .map_err(|_| InspectError::Allocation)?;
        for (request, row) in requests.iter().zip(rows) {
            check(budget, cancelled)?;
            keys.push(InsertKey {
                reference: row.reference,
                value: request
                    .values()
                    .get(column)
                    .ok_or(EditError::Unsupported(WriteBlocker::Column))?,
            });
        }

        // Qualification checks every alias and physical declaration against the
        // same source. A missing or unsupported owner never disappears.
        let plan = insertion::prepare(image, checked, column, physical, &keys, budget, cancelled)?;
        for prior in &plans {
            for page in plan.source_pages() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, prior.source_pages().len() as u64)
                    .map_err(InspectError::from)?;
                if prior.source_pages().contains(page) {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            }
        }
        let (plan, next) = plan.continue_with_append(image, checked, append, budget, cancelled)?;
        append = next;
        plans.push(plan);
    }
    Ok((plans, append))
}
