//! Independent numeric wire and request-carrier qualification.
use super::super::{Wire, reference};
use crate::EditError;
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

use super::floating;

#[cfg(test)]
mod tests;

const MARKER_BYTES: usize = size_of::<u8>();
const ROW_BYTES: usize = size_of::<u32>();
const MAX_LEAF_WIRE_BYTES: usize = MARKER_BYTES + size_of::<i64>() + ROW_BYTES;
const ASC_VALUE: u8 = 0x7f;
const DESC_VALUE: u8 = 0x80;
const ASC_NULL: u8 = 0;
const DESC_NULL: u8 = 0xff;
const ASC_TRUE: u8 = 0;
const ASC_FALSE: u8 = 0xff;
const TRUE_ATOM: i64 = -1;
const FALSE_ATOM: i64 = 0;

#[derive(Clone, Copy)]
pub(super) struct Profile {
    pub(super) kind: ScalarType,
    pub(super) width: usize,
    pub(super) descending: bool,
    pub required: bool,
}

impl Profile {
    pub(super) fn request(self, value: &Value) -> Result<Option<i64>, EditError> {
        let result = match (self.kind, value) {
            (_, Value::Null) if !self.required && self.kind != ScalarType::Boolean => None,
            (ScalarType::Boolean, Value::Boolean(true)) => Some(TRUE_ATOM),
            (ScalarType::Boolean, Value::Boolean(false)) => Some(FALSE_ATOM),
            (ScalarType::Byte, Value::Byte(value)) => Some(i64::from(*value)),
            (ScalarType::Int16, Value::Int16(value)) => Some(i64::from(*value)),
            (ScalarType::Int32, Value::Int32(value)) => Some(i64::from(*value)),
            (ScalarType::Int64, Value::Int64(value)) => Some(*value),
            (ScalarType::Currency, Value::Currency(value)) => Some(value.scaled()),
            (ScalarType::Single, Value::Single(value)) => Some(floating::single(value.to_bits())),
            (ScalarType::Double, Value::Double(value)) => Some(floating::double(value.to_bits())),
            (ScalarType::DateTime, Value::DateTime(value)) => {
                Some(floating::double(value.to_bits()))
            }
            _ => return Err(EditError::Preservation),
        };
        Ok(result)
    }

    pub(super) fn encode(self, value: Option<i64>, row: u32) -> Result<Wire, EditError> {
        let mut bytes = [0; MAX_LEAF_WIRE_BYTES];
        let key_length = if let Some(value) = value {
            let valid_atom = match self.kind {
                ScalarType::Byte => u8::try_from(value).is_ok(),
                ScalarType::Int16 => i16::try_from(value).is_ok(),
                ScalarType::Int32 | ScalarType::Single => i32::try_from(value).is_ok(),
                _ => true,
            };
            if !valid_atom {
                return Err(EditError::Preservation);
            }
            bytes[0] = ASC_VALUE;
            let bits = if self.kind == ScalarType::Boolean {
                match value {
                    TRUE_ATOM => u64::from(ASC_TRUE),
                    FALSE_ATOM => u64::from(ASC_FALSE),
                    _ => return Err(EditError::Preservation),
                }
            } else if self.kind == ScalarType::Byte {
                value as u64
            } else {
                (value as u64) ^ (1_u64 << (self.width * u8::BITS as usize - 1))
            };
            if self.kind == ScalarType::DateTime {
                floating::validate_datetime(bits)?;
            }
            let encoded = bits.to_be_bytes();
            bytes[MARKER_BYTES..MARKER_BYTES + self.width]
                .copy_from_slice(&encoded[encoded.len() - self.width..]);
            MARKER_BYTES + self.width
        } else {
            if self.required || self.kind == ScalarType::Boolean {
                return Err(EditError::Preservation);
            }
            MARKER_BYTES
        };
        if self.descending {
            for byte in &mut bytes[..key_length] {
                *byte = !*byte;
            }
        }
        bytes[key_length..key_length + ROW_BYTES].copy_from_slice(&row.to_be_bytes());
        Wire::read(&bytes[..key_length + ROW_BYTES])
    }

    pub(super) fn value(self, wire: &Wire) -> Result<Option<i64>, EditError> {
        let null_marker = if self.descending { DESC_NULL } else { ASC_NULL };
        let marker = if self.descending {
            DESC_VALUE
        } else {
            ASC_VALUE
        };
        let value = if wire.first() == Some(&null_marker) {
            if self.required
                || self.kind == ScalarType::Boolean
                || wire.len() != MARKER_BYTES + ROW_BYTES
            {
                return Err(EditError::Preservation);
            }
            None
        } else {
            if wire.first() != Some(&marker) || wire.len() != MARKER_BYTES + self.width + ROW_BYTES
            {
                return Err(EditError::Preservation);
            }
            let mut bits = 0_u64;
            for byte in &wire[MARKER_BYTES..MARKER_BYTES + self.width] {
                bits = (bits << u8::BITS) | u64::from(if self.descending { !*byte } else { *byte });
            }
            if self.kind == ScalarType::Boolean {
                Some(match bits {
                    value if value == u64::from(ASC_TRUE) => TRUE_ATOM,
                    value if value == u64::from(ASC_FALSE) => FALSE_ATOM,
                    _ => return Err(EditError::Preservation),
                })
            } else if self.kind == ScalarType::Byte {
                Some(bits as i64)
            } else {
                if self.kind == ScalarType::DateTime {
                    floating::validate_datetime(bits)?;
                }
                bits ^= 1_u64 << (self.width * u8::BITS as usize - 1);
                let shift = (size_of::<i64>() - self.width) * u8::BITS as usize;
                Some(((bits << shift) as i64) >> shift)
            }
        };
        if self.encode(value, reference(wire))? != *wire {
            return Err(EditError::Preservation);
        }
        Ok(value)
    }
}
