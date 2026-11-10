//! Original-value Short Text updates select actual stored presence without fallback.
mod proof;

use super::{
    historical_fixed::source::Source,
    normal_fitted::binding,
    record_placement::{audit, forward::Plan, owned::Reservations, space::Space},
    record_roots, record_serialization,
    rows::RowAssignment,
};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    format::{
        bytes::u16_at,
        data_type::SHORT_TEXT,
        page::PAGE_BYTES,
        row::{OFFSET_MASK, RELOCATED_MASK},
    },
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) struct Selected {
    ordinal: usize,
    column: usize,
    changed: bool,
}

impl RowAssignment for Selected {
    fn row(&self) -> usize {
        self.ordinal
    }
}

pub(super) fn bind(
    image: &NativeImage,
    source: &Source<'_>,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Selected, EditError> {
    if groups.len() != 1
        || groups[0].len() != 1
        || !matches!(groups[0], FittedColumnUpdates::Variable { .. })
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let bound = binding::all(source.facts(), groups, budget, cancelled)?;
    let column = *bound.columns.first().ok_or(EditError::Preservation)?;
    let schema = source.facts().table.schema();
    if schema.columns()[column].native_type() != SHORT_TEXT {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let update = groups[0].cell(0).ok_or(EditError::Preservation)?;
    if !matches!(update.value, Value::Text(_) | Value::Null) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let values = super::normal_fitted::original_values(
        image,
        source.facts(),
        update.row,
        &[column],
        budget,
        cancelled,
    )?;
    let original = values.first().ok_or(EditError::Preservation)?;
    if !binding::equal(original, update.expected, budget, cancelled)? {
        return Err(EditError::Conflict);
    }
    let changed = !binding::equal(original, update.value, budget, cancelled)?;
    let record = source
        .facts()
        .records
        .get(update.row)
        .ok_or(EditError::Conflict)?;
    record_serialization::historical_text::qualify(
        schema,
        column,
        &record.data,
        budget,
        cancelled,
    )?;
    Ok(Selected {
        ordinal: update.row,
        column,
        changed,
    })
}

impl NativeImage {
    /// Observe an actual Short Text target before choosing the separate request language.
    pub(crate) fn historical_short_text_updates_selected(
        &self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        let source = Source::capture(self, table, budget, cancelled)?;
        for group in groups {
            for field in source.facts().table.schema().columns() {
                check(budget, cancelled)?;
                if field.native_type() != SHORT_TEXT
                    || !field.name().eq_ignore_ascii_case(group.name())
                {
                    continue;
                }
                if let Some(cell) = group.cell(0) {
                    let record = source
                        .facts()
                        .records
                        .get(cell.row)
                        .ok_or(EditError::Conflict)?;
                    // Actual record framing is inspected before a consuming
                    // planner chooses Present or Absent; no failed operation
                    // is retried under another interpretation.
                    let count = usize::from(u16_at(&record.data, 0)?);
                    if count == 0 || count > source.facts().table.schema().layout.max_columns {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn plan_historical_short_text_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.historical_short_text_updates.plan").entered();
        let source = Source::capture(&self, table, budget, cancelled)?;
        let requests = [bind(&self, &source, groups, budget, cancelled)?];
        let schema = source.facts().table.schema();
        let captured = record_roots::capture(&self, schema, &requests, budget, cancelled)?;
        let root = captured.selected.first().ok_or(EditError::Conflict)?;
        if captured.selected.len() != 1 || !matches!(root.word & !OFFSET_MASK, 0 | RELOCATED_MASK) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let original = source
            .facts()
            .records
            .get(root.update.ordinal)
            .ok_or(EditError::Conflict)?;
        if original.offset != root.final_record.offset || original.data != root.final_record.data {
            return Err(EditError::Preservation);
        }
        let value = groups[0].cell(0).ok_or(EditError::Preservation)?.value;
        let changed = root.update.changed;
        let record = if changed {
            record_serialization::historical_text::build(
                schema,
                root.update.column,
                &original.data,
                value,
                budget,
                cancelled,
            )?
        } else {
            record_serialization::copy(&original.data, budget, cancelled)?
        };
        let mut after = record_serialization::copy(self.as_bytes(), budget, cancelled)?;
        if changed {
            let space = Space::capture_normal(&self, schema, &captured.owned, budget, cancelled)?;
            let mut reservations =
                Reservations::capture(&self, &space, &captured.owned, budget, cancelled)?;
            let reservation = reservations.reserve(&self, root, record.len(), budget, cancelled)?;
            let mut placement = Plan::new();
            placement.apply_reserved(
                &self,
                root,
                &record,
                &space,
                &reservation,
                budget,
                cancelled,
            )?;
            audit::pages(&placement, budget, cancelled)?;
            for page in &placement.pages {
                check(budget, cancelled)?;
                let at = usize::try_from(page.number)
                    .ok()
                    .and_then(|number| number.checked_mul(PAGE_BYTES))
                    .ok_or(EditError::Preservation)?;
                let target = after
                    .get_mut(at..at + PAGE_BYTES)
                    .ok_or(EditError::Preservation)?;
                if target != page.before.as_slice() || page.after.len() != PAGE_BYTES {
                    return Err(EditError::Preservation);
                }
                budget
                    .charge(Resource::Steps, PAGE_BYTES as u64)
                    .map_err(InspectError::from)?;
                target.copy_from_slice(&page.after);
            }
            audit::final_pages(&placement, &after, budget, cancelled)?;
            space.audit(self.as_bytes(), &after, budget, cancelled)?;
        }
        proof::prove(&self, table, groups, &after, budget, cancelled)?;
        drop(source);
        Ok(NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows: usize::from(changed),
        })
    }
}

#[cfg(test)]
mod tests;
