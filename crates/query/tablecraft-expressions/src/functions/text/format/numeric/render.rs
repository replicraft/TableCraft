//! Stream numeric placeholders and literal spans in their original mask positions.
use super::{Display, Kind, Tokens};
use crate::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        text::{format_buffer::FormatBuffer, output::TextOutput},
    },
};
use std::fmt::Write;

const GROUP_WIDTH: usize = 3;
const ZERO: &str = "0";

/// Numeric alignment counts digits only; literal text does not consume a slot.
struct WholeDigits<'a> {
    digits: &'a str,
    zero: bool,
    width: usize,
    actual: usize,
    position: usize,
    separator: Option<&'static str>,
}

impl WholeDigits<'_> {
    fn write_next(&mut self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        if let Some(separator) = self.separator
            && self.position > 0
            && (self.width - self.position).is_multiple_of(GROUP_WIDTH)
        {
            output.append(separator)?;
        }
        let index = self.position as i32 - (self.width - self.actual) as i32;
        output.append(digit(self.digits, index, self.zero))?;
        self.position += 1;
        Ok(())
    }
}

impl Display<'_> {
    pub fn write(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        let mut coefficient = FormatBuffer::new();
        write!(coefficient, "{}", self.magnitude.coefficient).map_err(|_| EvalError::TextLimit)?;
        let digits = coefficient.text()?;
        if self.magnitude.negative {
            output.append("-")?;
        }
        let actual = if self.magnitude.coefficient == 0 {
            0
        } else {
            self.magnitude.point.max(0) as usize
        };
        let width = actual.max(self.pattern.required_whole);
        let extra = width.saturating_sub(self.pattern.whole);
        let omitted = self.pattern.whole.saturating_sub(width);
        let mut whole = WholeDigits {
            digits,
            zero: self.magnitude.coefficient == 0,
            width,
            actual,
            position: 0,
            separator: self
                .pattern
                .grouping
                .then_some(if self.format == NumberFormat::DeDe {
                    "."
                } else {
                    ","
                }),
        };
        let mut exponent_buffer = FormatBuffer::new();
        let exponent_slots = self
            .pattern
            .scientific
            .as_ref()
            .map_or(0, |value| value.width());
        if self.pattern.scientific.is_some() {
            write!(exponent_buffer, "{}", self.exponent.unsigned_abs())
                .map_err(|_| EvalError::TextLimit)?;
        }
        let exponent_digits = exponent_buffer.text()?;
        let mut exponent = WholeDigits {
            digits: exponent_digits,
            zero: false,
            width: exponent_slots.max(exponent_digits.len()),
            actual: exponent_digits.len(),
            position: 0,
            separator: None,
        };
        let mut in_exponent = false;
        let mut first_exponent_digit = true;
        let fraction = self.fraction_width(digits, output)?;
        let mut whole_slot = 0;
        let mut fraction_slot = 0;
        let mut fractional = false;
        let mut tokens = Tokens::new(self.pattern.mask);
        while let Some(token) = tokens.next(&mut || output.step())? {
            if token.start == self.pattern.first {
                // Extra leading digits belong immediately before the first
                // numeric placeholder, after any literal prefix.
                for _ in 0..extra {
                    whole.write_next(output)?;
                }
            }
            match token.kind {
                Kind::Symbol('0' | '#') if in_exponent => {
                    if first_exponent_digit {
                        for _ in exponent_slots..exponent_digits.len() {
                            exponent.write_next(output)?;
                        }
                        first_exponent_digit = false;
                    }
                    exponent.write_next(output)?;
                }
                Kind::Symbol('0' | '#') if fractional => {
                    if fraction_slot < fraction {
                        output.append(digit(
                            digits,
                            self.magnitude.point + i32::from(fraction_slot),
                            self.magnitude.coefficient == 0,
                        ))?;
                    }
                    fraction_slot += 1;
                }
                Kind::Symbol('0' | '#') => {
                    if whole_slot >= omitted {
                        whole.write_next(output)?;
                    }
                    whole_slot += 1;
                }
                Kind::Symbol('.') => {
                    fractional = true;
                    output.append(if self.format == NumberFormat::DeDe {
                        ","
                    } else {
                        "."
                    })?;
                }
                Kind::Symbol(',') => {}
                Kind::Symbol('E' | 'e') => {
                    let scientific = self.pattern.scientific.as_ref().ok_or(EvalError::Domain)?;
                    scientific.skip_sign(&mut tokens, &mut || output.step())?;
                    scientific.write_prefix(self.exponent, output)?;
                    in_exponent = true;
                }
                Kind::Literal(text) => output.append(text)?,
                Kind::Symbol(symbol) => {
                    let mut encoded = [0; char::MAX.len_utf8()];
                    output.append(symbol.encode_utf8(&mut encoded))?;
                }
            }
        }
        Ok(())
    }

    fn fraction_width(&self, digits: &str, output: &mut impl TextOutput) -> Result<u8, EvalError> {
        let mut fraction = self.pattern.fraction;
        while fraction > self.pattern.required_fraction {
            output.step()?;
            if digit(
                digits,
                self.magnitude.point + i32::from(fraction) - 1,
                self.magnitude.coefficient == 0,
            ) != ZERO
            {
                break;
            }
            fraction -= 1;
        }
        Ok(fraction)
    }
}

fn digit(digits: &str, index: i32, zero: bool) -> &str {
    if zero || index < 0 {
        ZERO
    } else {
        digits
            .get(index as usize..index as usize + 1)
            .unwrap_or(ZERO)
    }
}
