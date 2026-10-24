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
    use tablecraft_core::limits::{LimitError, Resource};

    fn wide_row(count: u16) -> Vec<u8> {
        let variables = count - 1;
        let mut row = count.to_le_bytes().to_vec();
        row.extend([1, 0, 0, 0]);
        row.extend((0..variables).map(|position| position as u8));
        for offset in (6..=6 + variables).rev() {
            row.extend(offset.to_le_bytes());
        }
        row.extend(variables.to_le_bytes());
        row.resize(row.len() + usize::from(count).div_ceil(8), 0x55);
        row
    }

    fn wide_geometry(count: usize) -> Geometry {
        Geometry {
            fixed_end: 6,
            variable_slots: count - 1,
            minimum_variable_slots: count - 1,
        }
    }

    #[test]
    fn wide_rows_keep_unsigned_word_offsets_and_independent_mask_boundaries() {
        for count in [8, 9, 16, 17, 127, 128, 129, 254, 255] {
            let row = wide_row(count);
            let count = usize::from(count);
            let mut budget = ResourceBudget::new(8192, 0, 0);
            let layout = parse_geometry(
                &row,
                count,
                wide_geometry(count),
                &mut budget,
                Purpose::Values,
            )
            .unwrap();
            assert_eq!(layout.count, count);
            assert_eq!(layout.offsets, (6..6 + count).collect::<Vec<_>>());
            assert_eq!(layout.data_end, 5 + count);
            assert_eq!(row.len() - layout.mask_start, count.div_ceil(8));
        }

        let row = wide_row(255);
        assert_eq!(row.len(), 804);
        let layout = parse_geometry(
            &row,
            255,
            wide_geometry(255),
            &mut ResourceBudget::new(8192, 0, 0),
            Purpose::Values,
        )
        .unwrap();
        assert_eq!(layout.mask_start, 772);
        assert_eq!(layout.offsets[127], 133);
        assert_eq!(layout.offsets[128], 134);
        assert_eq!(layout.offsets[254], 260);
    }

    #[test]
    fn wide_row_corruption_and_offset_allocation_limits_fail_without_layouts() {
        for (offset, value) in [(260, 261_u16), (514, 0), (768, 5), (770, 253)] {
            let mut row = wide_row(255);
            row[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
            assert!(
                parse_geometry(
                    &row,
                    255,
                    wide_geometry(255),
                    &mut ResourceBudget::new(8192, 0, 0),
                    Purpose::Values,
                )
                .is_err()
            );
        }

        let row = wide_row(255);
        let bytes = 255 * size_of::<usize>() as u64;
        assert!(matches!(
            parse_geometry(
                &row,
                255,
                wide_geometry(255),
                &mut ResourceBudget::new(bytes - 1, 0, 0),
                Purpose::Values,
            ),
            Err(InspectError::Limit(LimitError {
                resource: Resource::Bytes,
                ..
            }))
        ));
    }

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
