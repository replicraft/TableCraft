//! Bounded high-precision IEEE rounding without an overflowing decimal factor.
use crate::EvalError;
use std::fmt::{self, Write};

// 10^-324 is smaller than half the least positive binary64 subnormal.
// Rounding to this or a finer decimal quantum cannot change its binary64 carrier.
const UNCHANGED_BINARY64_PLACES: u32 = 324;
const MAX_INTEGRAL_DIGITS: usize = f64::DIGITS as usize + 1;
const BUFFER_BYTES: usize = UNCHANGED_BINARY64_PLACES as usize + MAX_INTEGRAL_DIGITS + "-.".len();

pub(super) fn round(value: f64, places: u32) -> Result<f64, EvalError> {
    if places >= UNCHANGED_BINARY64_PLACES || value == 0.0 {
        return Ok(value);
    }

    // The caller already handles non-finite and integral-magnitude values.
    // At most 16 whole digits and 323 fractional digits reach the formatter.
    let mut buffer = Buffer {
        bytes: [0; BUFFER_BYTES],
        length: 0,
    };
    let places = places as usize;
    write!(buffer, "{value:.places$}").map_err(|_| EvalError::PrecisionLoss)?;
    let text = std::str::from_utf8(&buffer.bytes[..buffer.length])
        .map_err(|_| EvalError::PrecisionLoss)?;
    text.parse::<f64>().map_err(|_| EvalError::PrecisionLoss)
}

struct Buffer {
    bytes: [u8; BUFFER_BYTES],
    length: usize,
}

impl Write for Buffer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
        let destination = self.bytes.get_mut(self.length..end).ok_or(fmt::Error)?;
        destination.copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
