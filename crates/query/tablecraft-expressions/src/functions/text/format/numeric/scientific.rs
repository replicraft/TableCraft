//! Scientific display keeps the mantissa and decimal exponent independent.
use super::{Kind, TextOutput, Tokens, magnitude};
use crate::EvalError;

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
        if !matches!(tokens.remaining().as_bytes().first(), Some(b'0' | b'#')) {
            return Err(EvalError::UnsupportedExpression);
        }
        // The parent scanner counts all following exponent slots, including
        // those separated by literals, without allocating a token list.
        Ok(Self {
            marker,
            positive_sign,
            width: 0,
        })
    }

    pub fn add_placeholder(&mut self) {
        self.width += 1;
    }

    pub fn width(&self) -> usize {
        self.width
    }

    /// Consume the already validated one-byte ASCII exponent sign.
    pub fn skip_sign(
        &self,
        tokens: &mut Tokens<'_>,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<(), EvalError> {
        tokens.consume_ascii(size_of::<u8>(), step)
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

    pub fn write_prefix(
        &self,
        exponent: i32,
        output: &mut impl TextOutput,
    ) -> Result<(), EvalError> {
        output.append(if self.marker == 'E' { "E" } else { "e" })?;
        if exponent < 0 {
            output.append("-")?;
        } else if self.positive_sign {
            output.append("+")?;
        }
        Ok(())
    }
}
