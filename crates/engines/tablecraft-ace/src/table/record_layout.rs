//! Shared checked record geometry for scalar reads and same-width metadata patches.
use super::schema::TableSchema;
use crate::{
    InspectError,
    format::{
        bytes::u16_at,
        row::{MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(super) struct RecordLayout {
    pub count: usize,
    pub mask_start: usize,
    pub data_end: usize,
    pub offsets: Vec<usize>,
}
pub(super) fn parse(
    bytes: &[u8],
    schema: &TableSchema,
    budget: &mut ResourceBudget,
) -> Result<RecordLayout, InspectError> {
    let count = usize::from(u16_at(bytes, 0)?);
    let geometry = super::row_layout::geometry(schema, count)?;
    let mask_len = count.div_ceil(MASK_BITS);
    let mask_start = bytes
        .len()
        .checked_sub(mask_len)
        .ok_or(InspectError::Truncated)?;
    let (data_end, offsets) = if geometry.variable_slots == 0 {
        (mask_start, Vec::new())
    } else {
        let count_pos = mask_start
            .checked_sub(VARIABLE_COUNT_BYTES)
            .ok_or(InspectError::Truncated)?;
        let variables = usize::from(u16_at(bytes, count_pos)?);
        if variables < geometry.minimum_variable_slots || variables > geometry.variable_slots {
            return Err(InspectError::Unsupported);
        }
        let tail = count_pos
            .checked_sub(VARIABLE_OFFSET_BYTES * (variables + 1))
            .ok_or(InspectError::Truncated)?;
        budget.charge_allocation((variables + 1) as u64, std::mem::size_of::<usize>() as u64)?;
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(variables + 1)
            .map_err(|_| InspectError::Allocation)?;
        let mut previous = geometry.fixed_end;
        for i in 0..=variables {
            let next = usize::from(u16_at(bytes, count_pos - VARIABLE_OFFSET_BYTES * (i + 1))?);
            if next < previous || next > tail {
                return Err(InspectError::Malformed);
            }
            offsets.push(next);
            previous = next;
        }
        (tail, offsets)
    };
    if geometry.fixed_end > data_end {
        return Err(InspectError::Malformed);
    }
    Ok(RecordLayout {
        count,
        mask_start,
        data_end,
        offsets,
    })
}
