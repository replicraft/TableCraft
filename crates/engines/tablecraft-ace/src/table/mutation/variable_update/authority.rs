//! Record authority rebound exclusively from original metadata and requests.
#[cfg(test)]
mod tests;
use super::super::{
    EditError, SlotScalarUpdate, WriteBlocker,
    page_update::{roots, stage::Placement},
    slot::storage,
};
use super::{placement::Plan, record_proof};
use crate::{
    InspectError, NativeImage, TableCheck,
    format::reader::Reader,
    header::check,
    table::{record, record_layout},
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Operation<'a> {
    pub(super) table: &'a str,
    column: &'a str,
    updates: &'a [SlotScalarUpdate],
}

pub(super) struct ChangedRecord {
    pub(super) reference: u32,
    pub(super) root_word: u16,
    pub(super) root_range: Range<usize>,
    pub(super) final_offset: usize,
    pub(super) original_length: usize,
    pub(super) bytes: Vec<u8>,
}

pub(super) struct Verified {
    pub(super) records: Vec<ChangedRecord>,
}

impl<'a> Operation<'a> {
    pub(super) fn new(table: &'a str, column: &'a str, updates: &'a [SlotScalarUpdate]) -> Self {
        Self {
            table,
            column,
            updates,
        }
    }

    pub(super) fn prove(
        &self,
        image: &NativeImage,
        original: &TableCheck,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Verified, EditError> {
        check(budget, cancelled)?;
        let schema = original.schema();
        let column = schema
            .columns
            .iter()
            .position(|column| column.name().eq_ignore_ascii_case(self.column))
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        let field = &schema.columns[column];
        storage::qualify(field)?;

        let captured = roots::capture(image, schema, self.updates, budget, cancelled)?;

        budget
            .charge_allocation(self.updates.len() as u64, size_of::<ChangedRecord>() as u64)
            .map_err(InspectError::from)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(self.updates.len())
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
            storage::validate(field, root.update.expected())?;
            storage::validate(field, root.update.value())?;
            let layout = record_layout::parse(&root.final_record.data, schema, reader.budget)?;
            if layout.count != schema.layout.max_columns
                || layout.offsets.get(field.field.variable + 1).is_none()
            {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }

            let values = record::decode_selected(
                &root.final_record.data,
                schema,
                Some(&[column]),
                &mut reader,
            )?;
            if values.as_slice() != std::slice::from_ref(root.update.expected()) {
                return Err(EditError::Conflict);
            }

            if root.update.expected() == root.update.value() {
                continue;
            }

            let bytes = record_proof::reconstruct(
                schema,
                column,
                &root.final_record.data,
                root.update.value(),
                reader.budget,
                cancelled,
            )?;
            records.push(ChangedRecord {
                reference: root.reference,
                root_word: root.word,
                root_range: root.range,
                final_offset: root.final_record.offset,
                original_length: root.final_record.data.len(),
                bytes,
            });
        }

        Ok(Verified { records })
    }
}

impl Verified {
    pub(super) fn compare(
        &self,
        plan: &Plan,
        changed: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if changed != self.records.len() || plan.events.len() != self.records.len() {
            return Err(EditError::Preservation);
        }

        for (expected, event) in self.records.iter().zip(&plan.events) {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            if expected.reference != event.root
                || expected.root_word != event.word
                || expected.root_range != event.root_range
                || event.target.len() != expected.bytes.len()
                || (event.appended.is_some() != (expected.bytes.len() > expected.original_length))
                || (event.appended.is_none()
                    && event.target
                        != (expected.final_offset
                            ..expected.final_offset + expected.original_length))
            {
                return Err(EditError::Preservation);
            }

            let actual = plan.target(event.target.clone(), budget, cancelled)?;
            record_proof::equal(&expected.bytes, actual, budget, cancelled)?;
        }

        Ok(())
    }
}
