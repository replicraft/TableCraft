//! Reconstruct generation from original requests and TD bytes, never expanded rows.
use super::{
    AutoNumberRowInsert, IncrementingAutoNumber,
    counter::{Plan, Source},
};
use crate::table::mutation::insert::values::copy_value;
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    table::mutation::insert::{RowInsert, primary_column},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

// Independent wire replay: do not obtain the counter location or generated
// sequence from the producer's descriptor or arithmetic helper.
const SOURCE_LAST_ISSUED_OFFSET: usize = 20;
const LONG_AUTONUMBER_FLAGS: u8 = 0x07;
const GENERATED_FLAG_MASK: u8 = 0x44;

pub(in crate::table::mutation::insert) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub checked: &'a TableCheck,
    pub requests: &'a [AutoNumberRowInsert],
    pub policy: IncrementingAutoNumber,
    pub plan: &'a Plan,
    pub returned: &'a [i32],
}

impl Operation<'_> {
    /// Independently materialize complete expected rows for the existing record,
    /// LVAL, DATA and physical-index replay. Column selection and IDs are derived
    /// here; only allocation-free/carrier copying is shared with construction.
    pub(in crate::table::mutation::insert) fn rows(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RowInsert>, EditError> {
        let source = self.verify(budget, cancelled)?;
        let columns = self.checked.schema().columns().len();
        budget
            .charge_allocation(self.requests.len() as u64, size_of::<RowInsert>() as u64)
            .map_err(InspectError::from)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(self.requests.len())
            .map_err(|_| InspectError::Allocation)?;
        for (ordinal, request) in self.requests.iter().enumerate() {
            check(budget, cancelled)?;
            if request.values().len().checked_add(1) != Some(columns) {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            budget
                .charge_allocation(columns as u64, size_of::<Value>() as u64)
                .map_err(InspectError::from)?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(columns)
                .map_err(|_| InspectError::Allocation)?;
            let mut companions = request.values().iter();
            for position in 0..columns {
                check(budget, cancelled)?;
                let value = if position == source.column {
                    Value::Int32(generated(source.last, ordinal)?)
                } else {
                    copy_value(
                        companions.next().ok_or(EditError::Preservation)?,
                        budget,
                        cancelled,
                    )?
                };
                values.push(value);
            }
            if companions.next().is_some() {
                return Err(EditError::Preservation);
            }
            rows.push(RowInsert::new(values));
        }
        Ok(rows)
    }

    /// Apply only the independently derived counter transition to proof bytes.
    /// This is called before the complete candidate byte comparison.
    pub(in crate::table::mutation::insert) fn apply(
        &self,
        expected: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = self.verify(budget, cancelled)?;
        let after = final_counter(source.last, self.requests.len())?.to_le_bytes();
        let destination = expected
            .get_mut(source.offset..source.offset + size_of::<i32>())
            .ok_or(EditError::Preservation)?;
        if destination != source.before.as_slice() {
            return Err(EditError::Preservation);
        }
        destination.copy_from_slice(&after);
        Ok(())
    }

    fn verify(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Source, EditError> {
        let source = self.source(budget, cancelled)?;
        let after = final_counter(source.last, self.requests.len())?.to_le_bytes();
        if self.plan.source != source
            || self.plan.after != after
            || self.plan.values.len() != self.requests.len()
            || self.returned.len() != self.requests.len()
        {
            return Err(EditError::Preservation);
        }
        for ordinal in 0..self.requests.len() {
            check(budget, cancelled)?;
            let expected = generated(source.last, ordinal)?;
            if self.plan.values[ordinal] != expected || self.returned[ordinal] != expected {
                return Err(EditError::Preservation);
            }
        }
        Ok(source)
    }

    fn source(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Source, EditError> {
        let column = primary_column::resolve(self.checked, budget, cancelled)?;
        for (position, field) in self.checked.schema().columns().iter().enumerate() {
            check(budget, cancelled)?;
            if position == column {
                if field.scalar_type() != Some(ScalarType::Int32)
                    || field.storage_length() != size_of::<i32>()
                    || !field.fixed()
                    || field.flags() != LONG_AUTONUMBER_FLAGS
                    || field.extended_flags() != 0
                    || field.complex_type_id().is_some()
                {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
            } else if field.flags() & GENERATED_FLAG_MASK != 0 {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
        let offset = usize::try_from(self.checked.schema().definition_page())
            .ok()
            .and_then(|page| page.checked_mul(PAGE_BYTES))
            .and_then(|page| page.checked_add(SOURCE_LAST_ISSUED_OFFSET))
            .ok_or(InspectError::StructureLimit)?;
        let bytes = self
            .image
            .as_bytes()
            .get(offset..offset + size_of::<i32>())
            .ok_or(InspectError::Truncated)?;
        let before: [u8; size_of::<i32>()] =
            bytes.try_into().map_err(|_| InspectError::Truncated)?;
        let last = i32::from_le_bytes(before);
        if last != self.policy.expected_last() {
            return Err(EditError::Conflict);
        }
        Ok(Source {
            column,
            offset,
            before,
            last,
        })
    }
}

fn final_counter(last: i32, count: usize) -> Result<i32, EditError> {
    let count = i64::try_from(count).map_err(|_| EditError::AutoNumberExhausted)?;
    let value = i64::from(last)
        .checked_add(count)
        .ok_or(EditError::AutoNumberExhausted)?;
    i32::try_from(value).map_err(|_| EditError::AutoNumberExhausted)
}

fn generated(last: i32, ordinal: usize) -> Result<i32, EditError> {
    let count = ordinal
        .checked_add(1)
        .ok_or(EditError::AutoNumberExhausted)?;
    final_counter(last, count)
}
