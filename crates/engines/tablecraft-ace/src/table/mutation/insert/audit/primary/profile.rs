//! Independent numeric wire and request-carrier qualification.
use super::{Wire, reference};
use crate::{EditError, NativeIndex, table::schema::NativeColumn};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

#[cfg(test)]
mod tests;

const MARKER_BYTES: usize = size_of::<u8>();
const ROW_BYTES: usize = size_of::<u32>();
const MAX_LEAF_WIRE_BYTES: usize = MARKER_BYTES + size_of::<i64>() + ROW_BYTES;
const ASC_VALUE: u8 = 0x7f;
const DESC_VALUE: u8 = 0x80;
const ASC_NULL: u8 = 0;
const DESC_NULL: u8 = 0xff;

#[derive(Clone, Copy)]
pub(super) struct Profile {
    kind: ScalarType,
    width: usize,
    descending: bool,
    pub required: bool,
    pub unique: bool,
    pub ignore_nulls: bool,
}

impl Profile {
    pub(super) fn bind(index: &NativeIndex, field: &NativeColumn) -> Result<Self, EditError> {
        let [column] = index.columns() else {
            return Err(EditError::Preservation);
        };
        let kind = field.scalar_type().ok_or(EditError::Preservation)?;
        let width = match kind {
            ScalarType::Byte => size_of::<u8>(),
            ScalarType::Int16 => size_of::<i16>(),
            ScalarType::Int32 => size_of::<i32>(),
            ScalarType::Int64 | ScalarType::Currency => size_of::<i64>(),
            _ => return Err(EditError::Preservation),
        };
        let constraints = index.constraints().ok_or(EditError::Preservation)?;
        if column.number() != field.number()
            || (constraints.ignore_nulls() && constraints.required())
        {
            return Err(EditError::Preservation);
        }
        Ok(Self {
            kind,
            width,
            descending: column.descending(),
            required: constraints.required(),
            unique: constraints.unique(),
            ignore_nulls: constraints.ignore_nulls(),
        })
    }

    pub(super) fn request(self, value: &Value) -> Result<Option<i64>, EditError> {
        let result = match (self.kind, value) {
            (_, Value::Null) if !self.required => None,
            (ScalarType::Byte, Value::Byte(value)) => Some(i64::from(*value)),
            (ScalarType::Int16, Value::Int16(value)) => Some(i64::from(*value)),
            (ScalarType::Int32, Value::Int32(value)) => Some(i64::from(*value)),
            (ScalarType::Int64, Value::Int64(value)) => Some(*value),
            (ScalarType::Currency, Value::Currency(value)) => Some(value.scaled()),
            _ => return Err(EditError::Preservation),
        };
        Ok(result)
    }

    pub(super) fn encode(self, value: Option<i64>, row: u32) -> Result<Wire, EditError> {
        let mut bytes = [0; MAX_LEAF_WIRE_BYTES];
        let key_length = if let Some(value) = value {
            bytes[0] = ASC_VALUE;
            let bits = if self.kind == ScalarType::Byte {
                value as u64
            } else {
                (value as u64) ^ (1_u64 << (self.width * u8::BITS as usize - 1))
            };
            let encoded = bits.to_be_bytes();
            bytes[MARKER_BYTES..MARKER_BYTES + self.width]
                .copy_from_slice(&encoded[encoded.len() - self.width..]);
            MARKER_BYTES + self.width
        } else {
            if self.required {
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
            if self.required || wire.len() != MARKER_BYTES + ROW_BYTES {
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
            if self.kind == ScalarType::Byte {
                Some(bits as i64)
            } else {
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
