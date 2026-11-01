//! Actual Source records feed shared serialization and same-root placement once.
use super::super::{binding, stage as records};
use crate::{
    EditError, FittedColumnUpdates, NativeImage,
    header::check,
    table::mutation::{
        preserved_source::Source,
        record_placement::{audit, forward::Plan, space::Space},
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
    let mut plan = Plan::new();
    let mut applied = 0_usize;
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
            plan.apply(image, root, &record.after, &space, budget, cancelled)?;
            applied += 1;
        }
    }
    if applied != changed_rows {
        return Err(EditError::Preservation);
    }
    audit::pages(&plan, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(Prepared {
        plan,
        space,
        changed_rows,
    })
}
