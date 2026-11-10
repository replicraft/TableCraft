//! Current sparse declarations use one source-bound scalar replacement language.
mod proof;

use super::{
    fixed_records,
    historical_fixed::source::Source,
    normal_fitted::binding,
    record_placement::{audit, forward::Plan, owned::Reservations, space::Space},
    record_roots, record_serialization,
    rows::RowAssignment,
};
use crate::{
    EditError, FittedColumnUpdates, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage,
    WriteBlocker,
    format::{
        data_type::{INT32, SHORT_TEXT},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        row::{OFFSET_MASK, RELOCATED_MASK},
    },
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const SOURCE_PAGES: usize = 11;
const USER_DEFINITION: u32 = 7;

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
    if groups.len() != 1 || groups[0].len() != 1 {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let schema = source.facts().table.schema();
    let column = match &groups[0] {
        FittedColumnUpdates::Fixed { column, updates } => {
            let requests = [FixedColumnUpdates::new(schema.name(), column, updates)];
            let (bound, _) =
                fixed_records::binding::all_normal(source.facts(), &requests, budget, cancelled)?;
            bound.first().ok_or(EditError::Preservation)?.column
        }
        FittedColumnUpdates::Variable { .. } => {
            let bound = binding::all(source.facts(), groups, budget, cancelled)?;
            *bound.columns.first().ok_or(EditError::Preservation)?
        }
    };
    for index in source.facts().indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            if key.number() == schema.columns()[column].number() {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
    }
    let update = groups[0].cell(0).ok_or(EditError::Preservation)?;
    match (schema.columns()[column].native_type(), update.value) {
        (INT32, Value::Int32(_) | Value::Null)
            if matches!(groups[0], FittedColumnUpdates::Fixed { .. }) => {}
        (SHORT_TEXT, Value::Text(_) | Value::Null)
            if matches!(groups[0], FittedColumnUpdates::Variable { .. }) => {}
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
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
    record_serialization::current_rows::qualify(schema, column, &record.data, budget, cancelled)?;
    Ok(Selected {
        ordinal: update.row,
        column,
        changed,
    })
}

impl NativeImage {
    /// Original declaration history selects the current-row source before proof.
    pub fn select_native_current_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        check(budget, cancelled)?;
        let header = self.catalog().header();
        if header.format() != crate::FileFormat::Ace2016
            || header.subversion() != 0
            || header.encoded_pages()
            || self.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES
        {
            return Ok(false);
        }
        let schema = crate::inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        Ok(schema.definition_page() == USER_DEFINITION
            && schema.physical_layout().max_columns > schema.columns().len())
    }

    /// Prove actual current programs, constraints, ownership and record framing
    /// before presenting an eligible unindexed LONG or Short Text editor.
    pub fn check_native_current_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Source::capture_current_rows(self, table, budget, cancelled)?;
        let schema = source.facts().table.schema();
        for (column, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            let supported = (field.native_type() == INT32
                && field.fixed()
                && field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                && field.extended_flags() == 0)
                || (field.native_type() == SHORT_TEXT && !field.fixed());
            if !supported {
                continue;
            }
            let mut indexed = false;
            for index in source.facts().indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    indexed |= key.number() == field.number();
                }
            }
            if indexed {
                continue;
            }
            if field.fixed() {
                let groups = [FixedColumnUpdates::new(table, field.name(), &[])];
                fixed_records::binding::all_normal(source.facts(), &groups, budget, cancelled)?;
            } else {
                let groups = [FittedColumnUpdates::Variable {
                    column: field.name(),
                    updates: &[],
                }];
                binding::all(source.facts(), &groups, budget, cancelled)?;
            }
            for record in &source.facts().records {
                record_serialization::current_rows::qualify(
                    schema,
                    column,
                    &record.data,
                    budget,
                    cancelled,
                )?;
            }
            return Ok(());
        }
        Err(EditError::Unsupported(WriteBlocker::Column))
    }

    /// Requests retain the same metadata-selected language, without retrying
    /// a failed historical source under a more permissive interpretation.
    pub(crate) fn current_row_updates_selected(
        &self,
        table: &str,
        _groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        self.select_native_current_rows_table(table, budget, cancelled)
    }

    pub(crate) fn plan_current_row_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.current_row_updates.plan").entered();
        let source = Source::capture_current_rows(&self, table, budget, cancelled)?;
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
            record_serialization::current_rows::build(
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
