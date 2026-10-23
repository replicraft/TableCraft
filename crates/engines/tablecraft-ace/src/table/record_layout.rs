//! Visible-value framing and stricter complete-tail geometry for metadata patches.
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
    parse_for(bytes, schema, budget, Purpose::Complete)
}

// An ambiguous tail implies no surviving variable field was present: each such
// field would establish a nonzero minimum slot count. Read only known fixed
// payloads and the independently located null mask; absent fields remain absent.
pub(super) fn parse_values(
    bytes: &[u8],
    schema: &TableSchema,
    budget: &mut ResourceBudget,
) -> Result<RecordLayout, InspectError> {
    parse_for(bytes, schema, budget, Purpose::Values)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Complete,
    Values,
}

fn parse_for(
    bytes: &[u8],
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    purpose: Purpose,
) -> Result<RecordLayout, InspectError> {
    let count = usize::from(u16_at(bytes, 0)?);
    let geometry = super::row_layout::geometry(schema, count)?;
    parse_geometry(bytes, count, geometry, budget, purpose)
}

fn parse_geometry(
    bytes: &[u8],
    count: usize,
    geometry: super::row_layout::Geometry,
    budget: &mut ResourceBudget,
    purpose: Purpose,
) -> Result<RecordLayout, InspectError> {
    if geometry.ambiguous_tail() && purpose == Purpose::Complete {
        return Err(InspectError::Unsupported);
    }
    let mask_len = count.div_ceil(MASK_BITS);
    let mask_start = bytes
        .len()
        .checked_sub(mask_len)
        .ok_or(InspectError::Truncated)?;
    if geometry.ambiguous_tail() {
        if geometry.fixed_end > mask_start {
            return Err(InspectError::Malformed);
        }
        return Ok(RecordLayout {
            count,
            mask_start,
            data_end: geometry.fixed_end,
            offsets: Vec::new(),
        });
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::row_layout::Geometry;

    #[test]
    fn fixed_value_prefix_does_not_qualify_complete_tail_geometry() {
        let row = [2, 0, 2, 0, 0, 0, 9, 0, 0, 0, 3];
        let geometry = || Geometry {
            fixed_end: 6,
            variable_slots: 1,
            minimum_variable_slots: 0,
        };
        let mut budget = ResourceBudget::new(0, 0, 0);
        assert!(matches!(
            parse_geometry(&row, 2, geometry(), &mut budget, Purpose::Complete),
            Err(InspectError::Unsupported)
        ));
        let values = parse_geometry(&row, 2, geometry(), &mut budget, Purpose::Values).unwrap();
        assert_eq!(values.count, 2);
        assert_eq!(values.mask_start, 10);
        assert_eq!(values.data_end, 6);
        assert!(values.offsets.is_empty());
    }
}
