//! Explicit retained LONG zero-action planning does not widen single-cell writes.
mod proof;
#[cfg(test)]
mod tests;

use super::current_row_updates::{self, Family};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

impl NativeImage {
    pub(crate) fn plan_retained_empty_row_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.retained_empty_updates.plan").entered();
        qualify(&self, table, groups, budget, cancelled)?;
        let after = Family::Retained.copy_image(&self, budget, cancelled)?;
        proof::prove(&self, table, groups, &after, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows: 0,
        })
    }
}

fn qualify(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if groups.is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let source = Family::Retained.capture(image, table, budget, cancelled)?;
    let schema = source.facts().table.schema();
    for (position, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        let FittedColumnUpdates::Fixed { updates, .. } = group else {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        };
        if !updates.is_empty() {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let column = current_row_updates::retained_column_shape(
            &source,
            group,
            Family::Retained,
            budget,
            cancelled,
        )?;
        for prior in &groups[..position] {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    prior.name().len().max(group.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if prior.name().eq_ignore_ascii_case(group.name()) {
                return Err(EditError::Conflict);
            }
        }
        for index in source.facts().indexes.indexes() {
            for key in index.columns() {
                check(budget, cancelled)?;
                if key.number() == schema.columns()[column].number() {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            }
        }
        for record in &source.facts().records {
            check(budget, cancelled)?;
            Family::Retained.qualify_record(schema, column, &record.data, budget, cancelled)?;
        }
    }
    check(budget, cancelled)?;
    Ok(())
}
