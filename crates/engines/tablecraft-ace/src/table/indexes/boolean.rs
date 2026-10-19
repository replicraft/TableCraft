//! Native Boolean index atoms. Their private ordering is not SQL coercion.
use crate::InspectError;

const ASC_TRUE: u8 = 0x00;
const ASC_FALSE: u8 = 0xff;
const TRUE_ORDER: i64 = -1;
const FALSE_ORDER: i64 = 0;

pub(super) fn value_key(value: bool) -> i64 {
    if value { TRUE_ORDER } else { FALSE_ORDER }
}

// The shared codec has already undone descending complementation. Reject other
// bytes rather than treating arbitrary nonzero data as true or as an integer.
pub(super) fn decode(byte: u8) -> Result<i64, InspectError> {
    match byte {
        ASC_TRUE => Ok(TRUE_ORDER),
        ASC_FALSE => Ok(FALSE_ORDER),
        _ => Err(InspectError::Malformed),
    }
}

// The writer receives the same private two-value domain as the reader.
pub(super) fn encode(key: i64) -> Result<u8, InspectError> {
    match key {
        TRUE_ORDER => Ok(ASC_TRUE),
        FALSE_ORDER => Ok(ASC_FALSE),
        _ => Err(InspectError::Malformed),
    }
}
