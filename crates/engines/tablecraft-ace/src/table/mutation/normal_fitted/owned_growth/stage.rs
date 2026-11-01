//! Source records feed shared serialization and reserved owned-page placement.
use super::super::{binding, stage as records};
use crate::{
    EditError, FittedColumnUpdates, NativeImage,
    header::check,
    table::mutation::{
        preserved_source::Source,
        record_placement::{audit, forward::Plan, owned::Reservations, space::Space},
        record_roots,
        record_serialization::policy::RecordPolicy,
        rows::RowAssignment,
    },
};
use tablecraft_core::limits::ResourceBudget;

impl RowAssignment for binding::Selected {
    fn row(&self) -> usize {
        self.ordinal
    }
}

pub(super) struct Prepared {
    pub plan: Plan,
    pub space: Space,
    pub changed_rows: usize,
}

pub(super) fn prepare(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, EditError> {
    let bound = binding::all(source, groups, budget, cancelled)?;
    let schema = source.table.schema();
    let captured = record_roots::capture(image, schema, &bound.rows, budget, cancelled)?;
    let space = Space::capture_normal(image, schema, &captured.owned, budget, cancelled)?;
    let (records, changed_rows) = records::prepare_with(
        image,
        source,
        groups,
        RecordPolicy::GrowingPreferred,
        budget,
        cancelled,
    )?;
    let mut reservations =
        Reservations::capture(image, &space, &captured.owned, budget, cancelled)?;
    let mut operations = binding::vector(records.len(), budget)?;
    for root in &captured.selected {
        check(budget, cancelled)?;
        for record in &records {
            check(budget, cancelled)?;
            if root.final_record.offset != record.offset {
                continue;
            }
            if !binding::equal_bytes(&root.final_record.data, &record.before, budget, cancelled)? {
                return Err(EditError::Preservation);
            }
            let reservation =
                reservations.reserve(image, root, record.after.len(), budget, cancelled)?;
            operations.push((root, record, reservation));
        }
    }
    if operations.len() != changed_rows {
        return Err(EditError::Preservation);
    }
    // Every row now has a complete reservation. Only this loop copies/mutates
    // candidate page images; no failed placement can trigger another planner.
    let mut plan = Plan::new();
    for (root, record, reservation) in operations {
        plan.apply_reserved(
            image,
            root,
            &record.after,
            &space,
            &reservation,
            budget,
            cancelled,
        )?;
    }
    audit::pages(&plan, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(Prepared {
        plan,
        space,
        changed_rows,
    })
}
