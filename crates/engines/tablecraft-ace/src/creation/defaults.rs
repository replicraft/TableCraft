//! Bounded canonical Int32/NULL declaration text; no expression evaluator.
use super::CreateDatabaseError;
use crate::{InspectError, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const MAX_INT32_LITERAL_BYTES: usize = 11;
const DECIMAL_RADIX: u32 = 10;
const NULL_LITERAL: &[u8] = b"Null";

pub(super) struct Literal {
    bytes: [u8; MAX_INT32_LITERAL_BYTES],
    start: usize,
}

impl Literal {
    pub fn capture(
        value: &Value,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        check(budget, cancelled)?;
        let mut result = Self {
            bytes: [0; MAX_INT32_LITERAL_BYTES],
            start: MAX_INT32_LITERAL_BYTES,
        };
        let Value::Int32(value) = value else {
            if !matches!(value, Value::Null) {
                return Err(CreateDatabaseError::Carrier);
            }
            budget
                .charge(Resource::Steps, NULL_LITERAL.len() as u64)
                .map_err(InspectError::from)?;
            result.start -= NULL_LITERAL.len();
            result.bytes[result.start..].copy_from_slice(NULL_LITERAL);
            return Ok(result);
        };
        // unsigned_abs handles MIN without negating a signed value. At most
        // ten decimal digits plus one sign fit this fixed, nonallocating buffer.
        let mut magnitude = value.unsigned_abs();
        loop {
            check(budget, cancelled)?;
            result.start -= 1;
            result.bytes[result.start] = b'0' + (magnitude % DECIMAL_RADIX) as u8;
            magnitude /= DECIMAL_RADIX;
            if magnitude == 0 {
                break;
            }
        }
        if *value < 0 {
            check(budget, cancelled)?;
            result.start -= 1;
            result.bytes[result.start] = b'-';
        }
        check(budget, cancelled)?;
        Ok(result)
    }

    pub fn ascii(&self) -> &[u8] {
        &self.bytes[self.start..]
    }

    pub fn payload_bytes(&self) -> usize {
        self.ascii().len() * size_of::<u16>()
    }
}
