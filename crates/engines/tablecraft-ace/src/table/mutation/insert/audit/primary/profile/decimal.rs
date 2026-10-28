//! Independent ACE NUMERIC index segments, separate from 17-byte FIELD storage.
use super::super::Wire;
use crate::{EditError, format::reader::Reader};
use std::io::Cursor;
use tablecraft_core::values::Value;

#[cfg(test)]
mod tests;

const MAX_PRECISION: u8 = 28;
const MARKER_BYTES: usize = size_of::<u8>();
const SIGN_BYTES: usize = size_of::<u8>();
const MAGNITUDE_BYTES: usize = size_of::<u128>();
const KEY_BYTES: usize = MARKER_BYTES + SIGN_BYTES + MAGNITUDE_BYTES;
const ROW_BYTES: usize = size_of::<u32>();
const ASC_NULL: u8 = 0;
const DESC_NULL: u8 = 0xff;
const ASC_VALUE: u8 = 0x7f;
const DESC_VALUE: u8 = 0x80;
const POSITIVE: u8 = 0xff;
const NEGATIVE: u8 = 0;

#[derive(Clone, Copy)]
pub(super) struct Profile {
    precision: u8,
    scale: u8,
    descending: bool,
}

impl Profile {
    pub(super) fn new(precision: u8, scale: u8, descending: bool) -> Result<Self, EditError> {
        if !(1..=MAX_PRECISION).contains(&precision) || scale > precision {
            return Err(EditError::Preservation);
        }

        Ok(Self {
            precision,
            scale,
            descending,
        })
    }

    fn limit(self) -> u128 {
        10_u128.pow(u32::from(self.precision))
    }

    pub(super) fn encode(
        self,
        value: &Value,
        row: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Wire, EditError> {
        let mut bytes = [0; KEY_BYTES + ROW_BYTES];
        let length = match value {
            Value::Null => {
                bytes[0] = if self.descending { DESC_NULL } else { ASC_NULL };
                MARKER_BYTES
            }
            Value::Decimal(value) => {
                let coefficient = value.coefficient();
                if value.scale() != self.scale || coefficient.unsigned_abs() >= self.limit() {
                    return Err(EditError::Preservation);
                }

                bytes[0] = ASC_VALUE;
                bytes[MARKER_BYTES] = if coefficient < 0 { NEGATIVE } else { POSITIVE };

                let magnitude = coefficient.unsigned_abs().to_be_bytes();
                for (target, source) in bytes[MARKER_BYTES + SIGN_BYTES..KEY_BYTES]
                    .iter_mut()
                    .zip(magnitude)
                {
                    *target = if coefficient < 0 { !source } else { source };
                }

                if self.descending {
                    for byte in &mut bytes[..KEY_BYTES] {
                        *byte = !*byte;
                    }
                }
                KEY_BYTES
            }
            _ => return Err(EditError::Preservation),
        };

        bytes[length..length + ROW_BYTES].copy_from_slice(&row.to_be_bytes());
        Wire::read_paid(&bytes[..length + ROW_BYTES], reader)
    }

    pub(super) fn validate(self, bytes: &[u8]) -> Result<(bool, usize), EditError> {
        let marker = *bytes.first().ok_or(EditError::Preservation)?;
        if marker == if self.descending { DESC_NULL } else { ASC_NULL } {
            return Ok((true, MARKER_BYTES));
        }
        if marker
            != if self.descending {
                DESC_VALUE
            } else {
                ASC_VALUE
            }
        {
            return Err(EditError::Preservation);
        }
        let key = bytes.get(..KEY_BYTES).ok_or(EditError::Preservation)?;
        let sign = if self.descending {
            !key[MARKER_BYTES]
        } else {
            key[MARKER_BYTES]
        };
        let negative = match sign {
            POSITIVE => false,
            NEGATIVE => true,
            _ => return Err(EditError::Preservation),
        };
        let mut magnitude = [0; MAGNITUDE_BYTES];
        for (target, source) in magnitude.iter_mut().zip(&key[MARKER_BYTES + SIGN_BYTES..]) {
            let ascending = if self.descending { !*source } else { *source };
            *target = if negative { !ascending } else { ascending };
        }
        let magnitude = u128::from_be_bytes(magnitude);
        if magnitude >= self.limit() || (negative && magnitude == 0) {
            return Err(EditError::Preservation);
        }

        Ok((false, KEY_BYTES))
    }
}
