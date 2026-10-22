//! Scientific display keeps the mantissa and decimal exponent independent.
use super::{FormatBuffer, Kind, TextOutput, Tokens, magnitude};
use crate::EvalError;
use std::fmt::Write;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Scientific {
    marker: char,
    positive_sign: bool,
    width: usize,
}

impl Scientific {
    pub fn parse(
        marker: char,
        tokens: &mut Tokens<'_>,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<Self, EvalError> {
        let sign = tokens.next(step)?.ok_or(EvalError::UnsupportedExpression)?;
        let positive_sign = match sign.kind {
            Kind::Symbol('+') => true,
            Kind::Symbol('-') => false,
            _ => return Err(EvalError::UnsupportedExpression),
        };
        let width = tokens
            .remaining()
            .bytes()
            .take_while(|byte| matches!(byte, b'0' | b'#'))
            .count();
        if width == 0 {
            return Err(EvalError::UnsupportedExpression);
        }
        tokens.consume_ascii(width, step)?;
        Ok(Self {
            marker,
            positive_sign,
            width,
        })
    }

    pub fn normalize(
        &self,
        value: magnitude::Magnitude,
        whole: usize,
        fraction: u8,
        output: &mut impl TextOutput,
    ) -> Result<(magnitude::Magnitude, i32), EvalError> {
        let whole = i32::try_from(whole).map_err(|_| EvalError::TextLimit)?;
        let mut exponent = if value.coefficient == 0 {
            0
        } else {
            value.point - whole
        };
        let mut mantissa = magnitude::round_mantissa(value, whole, fraction, output)?;
        if mantissa.coefficient != 0 && mantissa.point > whole {
            let carry = mantissa.point - whole;
            mantissa.point -= carry;
            exponent += carry;
        }
        Ok((mantissa, exponent))
    }

    pub fn write(&self, exponent: i32, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.append(if self.marker == 'E' { "E" } else { "e" })?;
        if exponent < 0 {
            output.append("-")?;
        } else if self.positive_sign {
            output.append("+")?;
        }
        let mut buffer = FormatBuffer::new();
        write!(buffer, "{}", exponent.unsigned_abs()).map_err(|_| EvalError::TextLimit)?;
        let digits = buffer.text()?;
        for _ in digits.len()..self.width {
            output.append("0")?;
        }
        output.append(digits)
    }
}
