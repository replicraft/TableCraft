//! Complete expected-value staging before any candidate is published.
use super::super::{EditError, SlotScalarUpdate, WriteBlocker};
use super::{
    audit,
    forward::Plan,
    record,
    roots::{self, Root},
    space::Space,
};
use crate::{
    InspectError, NativeImage, TableCheck,
    format::{page::PAGE_BYTES, reader::Reader, row::reference_page},
    header::check,
    table::{indexes::SlotKeyUpdate, record as native_record, record_layout},
};
use std::ops::Range;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

struct Change<'u> {
    root: Root<'u>,
    values: Vec<Value>,
    record: Vec<u8>,
    target: Range<usize>,
}

pub(super) struct Staged<'u> {
    pub plan: Plan,
    pub space: Space,
    columns: Vec<usize>,
    changes: Vec<Change<'u>>,
}

impl Staged<'_> {
    pub(super) fn changed_rows(&self) -> usize {
        self.changes.len()
    }

    pub(super) fn keys(
        &self,
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<SlotKeyUpdate<'_>>, EditError> {
        budget
            .charge_allocation(
                self.changes.len() as u64,
                size_of::<SlotKeyUpdate<'_>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(self.changes.len())
            .map_err(|_| InspectError::Allocation)?;

        for change in &self.changes {
            check(budget, cancelled)?;
            result.push(SlotKeyUpdate {
                reference: change.root.reference,
                columns: &self.columns,
                before: &change.values,
                column,
                value: change.root.update.value(),
            });
        }
        Ok(result)
    }

    pub(super) fn audit(
        &self,
        image: &NativeImage,
        structural: &TableCheck,
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        audit::pages(&self.plan, budget, cancelled)?;
        let levels = self.plan.pages.len().checked_ilog2().unwrap_or(0) as usize + 1;
        for change in &self.changes {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    self.plan.pages.len() as u64 + levels as u64,
                )
                .map_err(InspectError::from)?;
            let number = u32::try_from(change.target.start / PAGE_BYTES)
                .map_err(|_| InspectError::Malformed)?;
            let page = self
                .plan
                .pages
                .iter()
                .find(|page| page.number == number)
                .ok_or(EditError::Preservation)?;
            let base = number as usize * PAGE_BYTES;
            let after = page
                .after
                .get(change.target.start - base..change.target.end - base)
                .ok_or(EditError::Preservation)?;
            if after != change.record {
                return Err(EditError::Preservation);
            }
            audit::record(
                image,
                structural.schema(),
                column,
                &change.root,
                after,
                budget,
                cancelled,
            )?;
        }
        Ok(())
    }
}

pub(super) fn build<'u>(
    image: &NativeImage,
    structural: &TableCheck,
    column: usize,
    updates: &'u [SlotScalarUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged<'u>, EditError> {
    let schema = structural.schema();
    let captured = roots::capture(image, schema, updates, budget, cancelled)?;
    let space = Space::capture(image, schema, &captured.owned, budget, cancelled)?;
    budget
        .charge_allocation(schema.columns.len() as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(schema.columns.len())
        .map_err(|_| InspectError::Allocation)?;

    for (position, field) in schema.columns.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (structural.indexes().len() * schema.columns.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if position == column
            || structural.indexes().iter().any(|index| {
                let keys = index.index().columns();
                keys.iter()
                    .any(|key| key.number() == schema.columns[column].number())
                    && keys.iter().any(|key| key.number() == field.number())
            })
        {
            columns.push(position);
        }
    }
    let selected = columns
        .iter()
        .position(|position| *position == column)
        .ok_or(InspectError::Malformed)?;
    budget
        .charge_allocation(updates.len() as u64, size_of::<Change<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut changes = Vec::new();
    changes
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut plan = Plan::new();
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };

    for root in captured.selected {
        reader.tick()?;
        let layout = record_layout::parse(&root.final_record.data, schema, reader.budget)?;
        let variable = schema.columns[column].field.variable;
        if layout.count != schema.layout.max_columns || layout.offsets.get(variable + 1).is_none() {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let values = native_record::decode_selected(
            &root.final_record.data,
            schema,
            Some(&columns),
            &mut reader,
        )?;
        if values.get(selected) != Some(root.update.expected()) {
            return Err(EditError::Conflict);
        }
        if root.update.expected() == root.update.value() {
            space.require_page(
                image,
                reference_page(root.reference),
                false,
                reader.budget,
                reader.cancelled,
            )?;
            let final_page = u32::try_from(root.final_record.offset / PAGE_BYTES)
                .map_err(|_| InspectError::Malformed)?;
            space.require_page(image, final_page, false, reader.budget, reader.cancelled)?;
            continue;
        }
        let record = record::build(
            schema,
            column,
            &root.final_record.data,
            root.update.value(),
            reader.budget,
            reader.cancelled,
        )?;
        let target = plan.apply(
            image,
            &root,
            &record,
            &space,
            reader.budget,
            reader.cancelled,
        )?;
        changes.push(Change {
            root,
            values,
            record,
            target,
        });
    }
    Ok(Staged {
        plan,
        space,
        columns,
        changes,
    })
}
