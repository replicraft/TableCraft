//! Explicit NULL-to-LONG growth of one absent field in a historical fixed row.
mod proof;

use super::{
    fixed_records::binding,
    historical_fixed::source::Source,
    record_placement::{audit, forward::Plan, owned::Reservations, space::Space},
    record_roots,
    record_serialization::{self, historical_fixed, historical_mixed},
    rows::RowAssignment,
};
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError, NativeEditPlan, NativeImage,
    WriteBlocker,
    format::{data_type::INT32, page::PAGE_BYTES, row::OFFSET_MASK},
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Fixed,
    Mixed,
}

pub(super) struct Selected {
    ordinal: usize,
    column: usize,
}

impl RowAssignment for Selected {
    fn row(&self) -> usize {
        self.ordinal
    }
}

pub(super) fn bind(
    source: &Source<'_>,
    groups: &[FixedColumnUpdates<'_>],
    layout: Layout,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Selected, EditError> {
    check(budget, cancelled)?;
    if groups.len() != 1 || groups[0].updates().len() != 1 {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    binding::all_normal(source.facts(), groups, budget, cancelled)?;
    let schema = source.facts().table.schema();
    let mut column = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.name().eq_ignore_ascii_case(groups[0].column()) {
            column = Some(position);
        }
    }
    let column = column.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    if schema.columns()[column].native_type() != INT32 {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let update = &groups[0].updates()[0];
    if !matches!(update.expected(), Value::Null) || !matches!(update.value(), Value::Int32(_)) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let original = source
        .facts()
        .records
        .get(update.row())
        .ok_or(EditError::Conflict)?;
    match layout {
        Layout::Fixed => {
            historical_fixed::qualify(schema, column, &original.data, budget, cancelled)?;
        }
        Layout::Mixed => {
            historical_mixed::qualify(schema, column, &original.data, budget, cancelled)?;
        }
    }
    Ok(Selected {
        ordinal: update.row(),
        column,
    })
}

impl NativeImage {
    /// Select the original absent-field shape before either historical planner.
    /// A positive selection still needs complete source and request admission.
    pub(crate) fn historical_number_materialization_selected(
        &self,
        table: &str,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        Ok(self
            .missing_historical_layout(table, groups, budget, cancelled)?
            .is_some())
    }

    pub(crate) fn historical_mixed_number_materialization_selected(
        &self,
        table: &str,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        Ok(matches!(
            self.missing_historical_layout(table, groups, budget, cancelled)?,
            Some(Layout::Mixed)
        ))
    }

    fn missing_historical_layout(
        &self,
        table: &str,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Layout>, EditError> {
        let source = Source::capture(self, table, budget, cancelled)?;
        for group in groups {
            for field in source.facts().table.schema().columns() {
                check(budget, cancelled)?;
                if !field.name().eq_ignore_ascii_case(group.column()) {
                    continue;
                }
                for update in group.updates() {
                    check(budget, cancelled)?;
                    let record = source
                        .facts()
                        .records
                        .get(update.row())
                        .ok_or(EditError::Conflict)?;
                    let count = usize::from(crate::format::bytes::u16_at(&record.data, 0)?);
                    if field.number() >= count {
                        return Ok(Some(
                            if source.facts().table.schema().layout.variable_slots == 0 {
                                Layout::Fixed
                            } else {
                                Layout::Mixed
                            },
                        ));
                    }
                }
            }
        }
        Ok(None)
    }

    pub(crate) fn plan_historical_number_materialization(
        self,
        table: &str,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        self.plan_historical_materialization(table, groups, Layout::Fixed, budget, cancelled)
    }

    pub(crate) fn plan_historical_mixed_number_materialization(
        self,
        table: &str,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        self.plan_historical_materialization(table, groups, Layout::Mixed, budget, cancelled)
    }

    fn plan_historical_materialization(
        self,
        table: &str,
        groups: &[FixedColumnUpdates<'_>],
        layout: Layout,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.historical_number_materialization.plan").entered();
        let source = Source::capture(&self, table, budget, cancelled)?;
        let selected = bind(&source, groups, layout, budget, cancelled)?;
        let requests = [selected];
        let schema = source.facts().table.schema();
        let captured = record_roots::capture(&self, schema, &requests, budget, cancelled)?;
        let root = captured.selected.first().ok_or(EditError::Conflict)?;
        if captured.selected.len() != 1 || root.word & !OFFSET_MASK != 0 {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let original = source
            .facts()
            .records
            .get(root.update.ordinal)
            .ok_or(EditError::Conflict)?;
        budget
            .charge(Resource::Steps, original.data.len() as u64)
            .map_err(InspectError::from)?;
        if original.offset != root.final_record.offset || original.data != root.final_record.data {
            return Err(EditError::Preservation);
        }
        let update: &FixedScalarUpdate = &groups[0].updates()[0];
        let record = match layout {
            Layout::Fixed => historical_fixed::build(
                schema,
                root.update.column,
                &original.data,
                update.value(),
                budget,
                cancelled,
            )?,
            Layout::Mixed => historical_mixed::build(
                schema,
                root.update.column,
                &original.data,
                update.value(),
                budget,
                cancelled,
            )?,
        };
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
        let mut after = record_serialization::copy(self.as_bytes(), budget, cancelled)?;
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
        proof::prove(&self, table, groups, layout, &after, budget, cancelled)?;
        drop(source);
        Ok(NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows: 1,
        })
    }
}
