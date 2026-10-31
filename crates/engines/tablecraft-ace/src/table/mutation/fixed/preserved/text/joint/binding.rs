//! Complete mixed target preflight before original records are composed.
use super::super::super::{joint::binding as fixed, name_work, source::Source};
use super::super::stage;
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, WriteBlocker,
    header::check,
    table::mutation::{rows, slot::storage},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) enum Kind {
    Text { projected: usize },
    Fixed(fixed::Bound),
}

pub(super) struct Bound {
    pub(super) column: usize,
    pub(super) kind: Kind,
}

pub(super) fn all(
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Bound>, usize), EditError> {
    check(budget, cancelled)?;
    if groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    if groups.len() < 2 {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    budget
        .charge_allocation(groups.len() as u64, size_of::<Bound>() as u64)
        .map_err(InspectError::from)?;
    let mut bound: Vec<Bound> = Vec::new();
    bound
        .try_reserve_exact(groups.len())
        .map_err(|_| InspectError::Allocation)?;
    let schema = source.table.schema();
    let mut assignments = 0_usize;
    let mut text_groups = 0_usize;
    for group in groups {
        check(budget, cancelled)?;
        let (name, count) = match group {
            FittedColumnUpdates::Fixed { column, updates } => (*column, updates.len()),
            FittedColumnUpdates::Variable { column, updates } => (*column, updates.len()),
        };
        assignments = assignments
            .checked_add(count)
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
        let mut selected = None;
        for (position, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            name_work(field.name(), name, budget)?;
            if field.name().eq_ignore_ascii_case(name) && selected.replace(position).is_some() {
                return Err(EditError::Preservation);
            }
        }
        let column = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        budget
            .charge(Resource::Steps, bound.len() as u64)
            .map_err(InspectError::from)?;
        if bound.iter().any(|prior| prior.column == column) {
            return Err(EditError::Conflict);
        }
        let kind = match group {
            FittedColumnUpdates::Fixed { updates, .. } => Kind::Fixed(fixed::qualify_selected(
                source, column, updates, budget, cancelled,
            )?),
            FittedColumnUpdates::Variable { updates, .. } => {
                text_groups += 1;
                let field = &schema.columns()[column];
                if text_groups != 1 || !field.is_ordinary_short_text() {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                storage::qualify(field)?;
                for dependency in &source.dependencies {
                    check(budget, cancelled)?;
                    if *dependency == field.number() {
                        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                    }
                }
                for index in source.indexes.indexes() {
                    for key in index.columns() {
                        check(budget, cancelled)?;
                        if key.number() == field.number() {
                            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                        }
                    }
                }
                let mut projected = None;
                for (position, candidate) in source.table.columns().iter().enumerate() {
                    check(budget, cancelled)?;
                    name_work(candidate.name(), field.name(), budget)?;
                    if candidate.name() == field.name() && projected.replace(position).is_some() {
                        return Err(EditError::Preservation);
                    }
                }
                rows::unique(updates, budget, cancelled)?;
                for update in *updates {
                    check(budget, cancelled)?;
                    if update.row() >= source.records.len() {
                        return Err(EditError::Conflict);
                    }
                    stage::validate(field, update.expected(), budget, cancelled)?;
                    stage::validate(field, update.value(), budget, cancelled)?;
                }
                Kind::Text {
                    projected: projected.ok_or(EditError::Preservation)?,
                }
            }
        };
        bound.push(Bound { column, kind });
    }
    if text_groups != 1 {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    check(budget, cancelled)?;
    Ok((bound, assignments))
}
