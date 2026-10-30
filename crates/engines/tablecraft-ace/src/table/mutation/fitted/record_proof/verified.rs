//! Fresh record authority and exact staged/index comparisons.
use super::super::{request, stage::Staged};
use crate::{
    EditError, InspectError, NativeImage, TableCheck,
    table::indexes::{IndexDataPatch, RowKeyUpdate, index_row_patches},
};
use std::ops::Range;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::table::mutation::fitted) struct ProvenRow {
    pub(in crate::table::mutation::fitted) reference: u32,
    pub(in crate::table::mutation::fitted) root_word: u16,
    pub(in crate::table::mutation::fitted) root_range: Range<usize>,
    pub(in crate::table::mutation::fitted) source_offset: usize,
    pub(in crate::table::mutation::fitted) before: Vec<u8>,
    pub(in crate::table::mutation::fitted) bytes: Vec<u8>,
    pub(in crate::table::mutation::fitted) original: Vec<Value>,
    pub(in crate::table::mutation::fitted) final_values: Vec<Value>,
}

pub(in crate::table::mutation::fitted) struct Verified {
    pub(in crate::table::mutation::fitted) checked: TableCheck,
    pub(in crate::table::mutation::fitted) columns: Vec<usize>,
    pub(in crate::table::mutation::fitted) projection: Vec<usize>,
    pub(in crate::table::mutation::fitted) records: Vec<ProvenRow>,
    pub(super) source_address: usize,
    pub(super) source_length: usize,
}

impl Verified {
    pub(in crate::table::mutation::fitted) fn require_source(
        &self,
        image: &NativeImage,
    ) -> Result<(), EditError> {
        if self.source_address != image.as_bytes().as_ptr() as usize
            || self.source_length != image.as_bytes().len()
        {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    pub(in crate::table::mutation::fitted) fn compare_staged(
        &self,
        staged: &Staged,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        request::check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (self.columns.len() + self.projection.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if self.columns != staged.affected
            || self.projection != staged.projection
            || self.records.len() != staged.records.len()
        {
            return Err(EditError::Preservation);
        }
        for (expected, actual) in self.records.iter().zip(&staged.records) {
            request::check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (expected.before.len() + expected.bytes.len()) as u64,
                )
                .map_err(InspectError::from)?;
            if expected.reference != actual.reference
                || expected.source_offset != actual.offset
                || expected.before != actual.before
                || expected.bytes != actual.after
                || !request::rows_equal(
                    &expected.original,
                    &actual.original_values,
                    budget,
                    cancelled,
                )?
                || !request::rows_equal(
                    &expected.final_values,
                    &actual.final_values,
                    budget,
                    cancelled,
                )?
            {
                return Err(EditError::Preservation);
            }
        }
        Ok(())
    }

    pub(in crate::table::mutation::fitted) fn keys(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RowKeyUpdate<'_>>, EditError> {
        budget
            .charge_allocation(
                self.records.len() as u64,
                size_of::<RowKeyUpdate<'_>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(self.records.len())
            .map_err(|_| InspectError::Allocation)?;
        for record in &self.records {
            request::check(budget, cancelled)?;
            keys.push(RowKeyUpdate {
                reference: record.reference,
                columns: &self.projection,
                before: &record.original,
                after: &record.final_values,
            });
        }
        Ok(keys)
    }

    pub(in crate::table::mutation::fitted) fn prove_indexes(
        &self,
        image: &NativeImage,
        indexes: &[IndexDataPatch],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.require_source(image)?;
        let keys = self.keys(budget, cancelled)?;
        let expected = index_row_patches(
            image,
            self.checked.schema(),
            self.checked.indexes(),
            &self.columns,
            &keys,
            budget,
            cancelled,
        )?;
        if expected.len() != indexes.len() {
            return Err(EditError::Preservation);
        }
        for (expected, actual) in expected.iter().zip(indexes) {
            request::check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (expected.before.len() + expected.after.len()) as u64,
                )
                .map_err(InspectError::from)?;
            if expected.offset != actual.offset
                || expected.before != actual.before
                || expected.after != actual.after
            {
                return Err(EditError::Preservation);
            }
        }
        Ok(())
    }
}

impl crate::table::mutation::variable_update::map_audit::record::RecordImage for ProvenRow {
    fn reference(&self) -> u32 {
        self.reference
    }
    fn root_word(&self) -> u16 {
        self.root_word
    }
    fn root_range(&self) -> &Range<usize> {
        &self.root_range
    }
    fn final_offset(&self) -> usize {
        self.source_offset
    }
    fn original_length(&self) -> usize {
        self.before.len()
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
