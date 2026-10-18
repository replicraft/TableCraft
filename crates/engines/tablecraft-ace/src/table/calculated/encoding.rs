use super::super::schema::NativeColumn;
use crate::{
    InspectError, ReadProfile,
    format::{
        data_type,
        layout::{CALCULATED_EXT_MASK, UPDATABLE_FLAG},
    },
};

// Reviewed CalculatedColumnUtil wrapper and fixed-result capacity. No expression
// executes; unknown header/trailer variants remain outside this stored profile.
const FIXED_RESULT_CAPACITY_BYTES: usize = 39;
const LENGTH_OFFSET: usize = 16;
const DATA_OFFSET: usize = LENGTH_OFFSET + size_of::<u32>();
const TRAILER_BYTES: usize = 3;
const WRAPPER_BYTES: usize = DATA_OFFSET + TRAILER_BYTES;

pub(in crate::table) fn supported(column: &NativeColumn) -> bool {
    matches!(
        column.read_profile(),
        ReadProfile::Ace2016 | ReadProfile::Ace2019
    ) && column.native_type() == data_type::INT32
        && !column.fixed()
        && column.storage_length() == FIXED_RESULT_CAPACITY_BYTES
        && column.flags() == UPDATABLE_FLAG
        && column.extended_flags() == CALCULATED_EXT_MASK
}

pub(in crate::table) fn decode(bytes: &[u8]) -> Result<Option<i32>, InspectError> {
    if bytes.len() < WRAPPER_BYTES {
        return Err(InspectError::Malformed);
    }
    if bytes[..LENGTH_OFFSET].iter().any(|byte| *byte != 0) {
        return Err(InspectError::Unsupported);
    }
    let length = crate::format::bytes::u32_at(bytes, LENGTH_OFFSET)? as usize;
    if !matches!(length, 0 | data_type::INT32_BYTES) || bytes.len() != WRAPPER_BYTES + length {
        return Err(InspectError::Malformed);
    }
    if bytes[DATA_OFFSET + length..].iter().any(|byte| *byte != 0) {
        return Err(InspectError::Unsupported);
    }
    if length == 0 {
        return Ok(None);
    }
    Ok(Some(i32::from_le_bytes(
        bytes[DATA_OFFSET..DATA_OFFSET + length]
            .try_into()
            .map_err(|_| InspectError::Malformed)?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_wrapped_int32_and_explicit_null_are_distinct_from_unknown_states() {
        let golden = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0xd8, 0xff, 0xff, 0xff, 0,
            0, 0,
        ];
        assert_eq!(decode(&golden), Ok(Some(-40)));
        assert_eq!(decode(&[0; 23]), Ok(None));
        for end in 0..golden.len() {
            assert!(decode(&golden[..end]).is_err());
        }
        for at in [0, 15, 16, 24, 26] {
            let mut changed = golden;
            changed[at] ^= 1;
            assert!(decode(&changed).is_err());
        }
        let mut longer = golden.to_vec();
        longer.push(0);
        assert!(decode(&longer).is_err());
    }
}
