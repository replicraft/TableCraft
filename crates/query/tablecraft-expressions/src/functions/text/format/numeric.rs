//! Custom fixed numeric masks reuse the shared bounded Decimal/IEEE magnitude.
use super::lexer::{self, Kind, Tokens};
use crate::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        text::{format_buffer::FormatBuffer, numeric_format::magnitude, output::TextOutput},
    },
};
use std::fmt::Write;
use tablecraft_core::values::Value;

const GROUP_WIDTH: usize = 3;
const PERCENT_SHIFT: u32 = 2;
const ZERO: &str = "0";

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Pattern<'a> {
    prefix: &'a str,
    suffix: &'a str,
    required_whole: usize,
    fraction: u8,
    required_fraction: u8,
    grouping: bool,
    decimal: bool,
    percent: bool,
}

pub(super) struct Display<'a> {
    pattern: Pattern<'a>,
    magnitude: magnitude::Magnitude,
    format: NumberFormat,
}

impl<'a> Pattern<'a> {
    pub fn parse(
        mask: &'a str,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<Option<Self>, EvalError> {
        let mut tokens = Tokens::new(mask);
        let mut first = None;
        let mut end = 0;
        let mut body_ended = false;
        let mut required_whole = 0;
        let mut fraction = 0u8;
        let mut required_fraction = 0;
        let mut decimal = false;
        let mut grouping = false;
        let mut percent = false;
        let mut trailing_comma = false;
        let mut mandatory_whole = false;
        while let Some(token) = tokens.next(step)? {
            match token.kind {
                Kind::Symbol(symbol @ ('0' | '#' | '.' | ',')) => {
                    if body_ended {
                        return Err(EvalError::UnsupportedExpression);
                    }
                    first.get_or_insert(token.start);
                    end = token.end;
                    match symbol {
                        '0' | '#' => {
                            trailing_comma = false;
                            if decimal {
                                fraction = fraction.checked_add(1).ok_or(EvalError::TextLimit)?;
                                if symbol == '0' {
                                    required_fraction = fraction;
                                }
                            } else if symbol == '0' {
                                mandatory_whole = true;
                                required_whole += 1;
                            } else if mandatory_whole {
                                return Err(EvalError::UnsupportedExpression);
                            }
                        }
                        '.' if !decimal && !trailing_comma => decimal = true,
                        ',' if !decimal && token.start != first.unwrap_or(token.start) => {
                            grouping = true;
                            trailing_comma = true;
                        }
                        _ => return Err(EvalError::UnsupportedExpression),
                    }
                }
                Kind::Symbol('%') => {
                    if percent {
                        return Err(EvalError::UnsupportedExpression);
                    }
                    percent = true;
                    body_ended |= first.is_some();
                }
                Kind::Literal(_) => body_ended |= first.is_some(),
                Kind::Symbol(symbol) if lexer::literal(symbol) => body_ended |= first.is_some(),
                _ => return Err(EvalError::UnsupportedExpression),
            }
        }
        if trailing_comma {
            return Err(EvalError::UnsupportedExpression);
        }
        let Some(first) = first else {
            return Ok(None);
        };
        Ok(Some(Self {
            prefix: &mask[..first],
            suffix: &mask[end..],
            required_whole,
            fraction,
            required_fraction,
            grouping,
            decimal,
            percent,
        }))
    }

    pub fn resolve(
        self,
        value: &Value,
        automatic_negative: bool,
        format: NumberFormat,
        output: &mut impl TextOutput,
    ) -> Result<Display<'a>, EvalError> {
        let shift = if self.percent { PERCENT_SHIFT } else { 0 };
        let mut magnitude = magnitude::prepare(value, self.fraction, shift, output)?;
        magnitude.negative &= automatic_negative;
        if self.grouping && format.group().is_none() {
            return Err(EvalError::UnsupportedExpression);
        }
        Ok(Display {
            pattern: self,
            magnitude,
            format,
        })
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
        literals(self.pattern.prefix, output)?;
        let actual_whole = if self.magnitude.coefficient == 0 {
            0
        } else {
            self.magnitude.point.max(0) as usize
        };
        let whole = actual_whole.max(self.pattern.required_whole);
        for position in 0..whole {
            if self.pattern.grouping
                && position > 0
                && (whole - position).is_multiple_of(GROUP_WIDTH)
            {
                output.append(if self.format == NumberFormat::DeDe {
                    "."
                } else {
                    ","
                })?;
            }
            let index = position as i32 - (whole - actual_whole) as i32;
            output.append(digit(digits, index, self.magnitude.coefficient == 0))?;
        }
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
        if self.pattern.decimal {
            output.append(if self.format == NumberFormat::DeDe {
                ","
            } else {
                "."
            })?;
        }
        for position in 0..fraction {
            output.append(digit(
                digits,
                self.magnitude.point + i32::from(position),
                self.magnitude.coefficient == 0,
            ))?;
        }
        literals(self.pattern.suffix, output)
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

pub(super) fn literals(mask: &str, output: &mut impl TextOutput) -> Result<(), EvalError> {
    let mut tokens = Tokens::new(mask);
    while let Some(token) = tokens.next(&mut || output.step())? {
        match token.kind {
            Kind::Literal(text) => output.append(text)?,
            Kind::Symbol('%') => output.append("%")?,
            Kind::Symbol(symbol) if lexer::literal(symbol) => {
                let mut buffer = [0; char::MAX.len_utf8()];
                output.append(symbol.encode_utf8(&mut buffer))?;
            }
            _ => return Err(EvalError::UnsupportedExpression),
        }
    }
    Ok(())
}
