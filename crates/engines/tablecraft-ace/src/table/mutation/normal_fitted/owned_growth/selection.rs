//! Original physical capacity selects one planner before either stages records.
use super::super::{
    binding::{self, Target},
    original_values,
};
use crate::{
    EditError, FittedColumnUpdates, NativeImage, WriteBlocker,
    header::check,
    table::{mutation::record_serialization::variables, record_layout},
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table::mutation) fn requires_growth(
    image: &NativeImage,
    source: &crate::table::mutation::preserved_source::Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let bound = binding::all(source, groups, budget, cancelled)?;
    super::super::inline::qualify(image, source, groups, &bound, budget, cancelled)?;
    let schema = source.table.schema();
    let mut growth = false;
    for selected in &bound.rows {
        check(budget, cancelled)?;
        let original = source
            .records
            .get(selected.ordinal)
            .ok_or(EditError::Conflict)?;
        let frame = record_layout::parse(&original.data, schema, budget)?;
        let values = original_values(
            image,
            source,
            selected.ordinal,
            &bound.columns,
            budget,
            cancelled,
        )?;
        let mut removed = 0_usize;
        let mut required = 0_usize;
        for assignment in &bound.assignments[selected.start..selected.end] {
            check(budget, cancelled)?;
            let cell = groups[assignment.group]
                .cell(assignment.index)
                .ok_or(EditError::Preservation)?;
            let actual = binding::value(&bound, &values, assignment.group, budget, cancelled)?;
            if !binding::equal(actual, cell.expected, budget, cancelled)? {
                return Err(EditError::Conflict);
            }
            if binding::equal(cell.expected, cell.value, budget, cancelled)? {
                continue;
            }
            let Target::Variable { column } = bound.targets[assignment.group] else {
                continue;
            };
            let field = &schema.columns()[column];
            let slot = field.field.variable;
            let span = frame
                .offsets
                .get(slot + 1)
                .ok_or(EditError::Preservation)?
                .checked_sub(*frame.offsets.get(slot).ok_or(EditError::Preservation)?)
                .ok_or(EditError::Preservation)?;
            removed = removed.checked_add(span).ok_or(EditError::Preservation)?;
            required = required
                .checked_add(variables::minimum_length(
                    field, cell.value, budget, cancelled,
                )?)
                .ok_or(EditError::Preservation)?;
        }
        let preserved = frame
            .offsets
            .last()
            .ok_or(EditError::Preservation)?
            .checked_sub(removed)
            .ok_or(EditError::Preservation)?;
        let capacity = frame
            .data_end
            .checked_sub(preserved)
            .ok_or(EditError::Preservation)?;
        growth |= required > capacity;
    }
    if growth && bound.has_long_targets {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    if growth {
        super::require_native_catalog(image, budget, cancelled)?;
    }
    check(budget, cancelled)?;
    Ok(growth)
}
