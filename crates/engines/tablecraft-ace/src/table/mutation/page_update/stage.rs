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
    InspectError, NativeImage, TableCheck, TableSchema,
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
    root: Root<'u, SlotScalarUpdate>,
    values: Vec<Value>,
    record: Vec<u8>,
    target: Range<usize>,
}

pub(in crate::table::mutation) struct Staged<'u, P> {
    pub plan: P,
    pub space: Space,
    facts: KeyFacts<'u>,
}

pub(in crate::table::mutation) struct KeyFacts<'u> {
    columns: Vec<usize>,
    changes: Vec<Change<'u>>,
}

impl KeyFacts<'_> {
    pub(in crate::table::mutation) fn changed_rows(&self) -> usize {
        self.changes.len()
    }

    pub(in crate::table::mutation) fn keys(
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
}

impl<'u, P: Placement> Staged<'u, P> {
    pub(in crate::table::mutation) fn changed_rows(&self) -> usize {
        self.facts.changed_rows()
    }

    pub(in crate::table::mutation) fn keys(
        &self,
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<SlotKeyUpdate<'_>>, EditError> {
        self.facts.keys(column, budget, cancelled)
    }

    pub(in crate::table::mutation) fn into_parts(self) -> (P, KeyFacts<'u>, Space) {
        (self.plan, self.facts, self.space)
    }

    pub(in crate::table::mutation) fn audit(
        &self,
        image: &NativeImage,
        structural: &TableCheck,
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.plan.audit(budget, cancelled)?;
        for change in &self.facts.changes {
            check(budget, cancelled)?;
            let after = self.plan.target(change.target.clone(), budget, cancelled)?;
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
) -> Result<Staged<'u, Plan>, EditError> {
    build_with(
        image,
        structural,
        column,
        updates,
        Plan::new(),
        budget,
        cancelled,
    )
}

pub(in crate::table::mutation) fn build_with<'u, P: Placement>(
    image: &NativeImage,
    structural: &TableCheck,
    column: usize,
    updates: &'u [SlotScalarUpdate],
    mut plan: P,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged<'u, P>, EditError> {
    let schema = structural.schema();
    let captured = roots::capture(image, schema, updates, budget, cancelled)?;
    let space = plan.capture_space(image, schema, &captured.owned, budget, cancelled)?;
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
        facts: KeyFacts { columns, changes },
    })
}

/// Real placement consumers share checked roots, record serialization and keys.
pub(in crate::table::mutation) trait Placement {
    fn capture_space(
        &mut self,
        image: &NativeImage,
        schema: &TableSchema,
        owned: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Space, EditError> {
        Space::capture(image, schema, owned, budget, cancelled)
    }

    fn apply(
        &mut self,
        image: &NativeImage,
        root: &Root<'_, SlotScalarUpdate>,
        record: &[u8],
        space: &Space,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Range<usize>, EditError>;

    fn audit(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError>;

    fn target(
        &self,
        range: Range<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&[u8], EditError>;
}

impl Placement for Plan {
    fn apply(
        &mut self,
        image: &NativeImage,
        root: &Root<'_, SlotScalarUpdate>,
        record: &[u8],
        space: &Space,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Range<usize>, EditError> {
        Plan::apply(self, image, root, record, space, budget, cancelled)
    }

    fn audit(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        audit::pages(self, budget, cancelled)
    }

    fn target(
        &self,
        range: Range<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&[u8], EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.pages.len() as u64)
            .map_err(InspectError::from)?;
        let number = range.start / PAGE_BYTES;
        let page = self
            .pages
            .iter()
            .find(|page| page.number as usize == number)
            .ok_or(EditError::Preservation)?;
        let base = number * PAGE_BYTES;
        page.after
            .get(range.start - base..range.end - base)
            .ok_or(EditError::Preservation)
    }
}
