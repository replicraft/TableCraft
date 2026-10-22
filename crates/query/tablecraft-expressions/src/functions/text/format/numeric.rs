//! Custom numeric masks reuse the shared bounded Decimal/IEEE display magnitude.
mod render;
mod scientific;
use super::lexer::{self, Kind, Tokens};
use crate::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        text::{numeric_format::magnitude, output::TextOutput},
    },
};
use tablecraft_core::values::Value;

const PERCENT_SHIFT: i32 = 2;
const COMMA_SHIFT: i32 = 3;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Pattern<'a> {
    mask: &'a str,
    first: usize,
    required_whole: usize,
    fraction: u8,
    required_fraction: u8,
    grouping: bool,
    percent: bool,
    scaling: i32,
    whole: usize,
    scientific: Option<scientific::Scientific>,
}

pub(super) struct Display<'a> {
    pattern: Pattern<'a>,
    magnitude: magnitude::Magnitude,
    format: NumberFormat,
    exponent: i32,
}

impl<'a> Pattern<'a> {
    pub fn parse(
        mask: &'a str,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<Option<Self>, EvalError> {
        let mut tokens = Tokens::new(mask);
        let mut first = None;
        let mut first_required_whole = None;
        let mut fraction = 0u8;
        let mut required_fraction = 0;
        let mut decimal = false;
        let mut grouping = false;
        let mut percent = false;
        let mut pending_commas = 0;
        let mut scaling = 0;
        let mut whole = 0;
        let mut scientific: Option<scientific::Scientific> = None;
        while let Some(token) = tokens.next(step)? {
            match token.kind {
                Kind::Symbol(symbol @ ('0' | '#' | '.' | ',')) => {
                    if let Some(scientific) = &mut scientific {
                        match symbol {
                            '0' | '#' => scientific.add_placeholder(),
                            _ => return Err(EvalError::UnsupportedExpression),
                        }
                        continue;
                    }
                    first.get_or_insert(token.start);
                    match symbol {
                        '0' | '#' => {
                            if pending_commas > 0 {
                                grouping = true;
                                pending_commas = 0;
                            }
                            if decimal {
                                fraction = fraction.checked_add(1).ok_or(EvalError::TextLimit)?;
                                if symbol == '0' {
                                    required_fraction = fraction;
                                }
                            } else {
                                if symbol == '0' {
                                    first_required_whole.get_or_insert(whole);
                                }
                                whole += 1;
                            }
                        }
                        '.' if !decimal => {
                            scaling = pending_commas * COMMA_SHIFT;
                            pending_commas = 0;
                            decimal = true;
                        }
                        ',' if !decimal && token.start != first.unwrap_or(token.start) => {
                            pending_commas += 1;
                        }
                        _ => return Err(EvalError::UnsupportedExpression),
                    }
                }
                Kind::Symbol(marker @ ('E' | 'e'))
                    if first.is_some() && scientific.is_none() && whole > 0 =>
                {
                    scientific = Some(scientific::Scientific::parse(marker, &mut tokens, step)?);
                }
                Kind::Symbol('%') => {
                    if percent {
                        return Err(EvalError::UnsupportedExpression);
                    }
                    percent = true;
                }
                Kind::Literal(_) => {}
                Kind::Symbol(symbol) if lexer::literal(symbol) => {}
                _ => return Err(EvalError::UnsupportedExpression),
            }
        }
        scaling += pending_commas * COMMA_SHIFT;
        let Some(first) = first else {
            return Ok(None);
        };
        Ok(Some(Self {
            mask,
            first,
            required_whole: first_required_whole.map_or(0, |first| whole - first),
            fraction,
            required_fraction,
            grouping,
            percent,
            scaling,
            whole,
            scientific,
        }))
    }

    pub fn resolve(
        self,
        value: &Value,
        automatic_negative: bool,
        format: NumberFormat,
        output: &mut impl TextOutput,
    ) -> Result<Display<'a>, EvalError> {
        let shift = if self.percent { PERCENT_SHIFT } else { 0 } - self.scaling;
        let mut magnitude = if self.scientific.is_some() {
            magnitude::unrounded(value, shift, output)?
        } else {
            magnitude::prepare_shifted(value, self.fraction, shift, output)?
        };
        let mut exponent = 0;
        if let Some(scientific) = &self.scientific {
            (magnitude, exponent) =
                scientific.normalize(magnitude, self.whole, self.fraction, output)?;
        }
        magnitude.negative &= automatic_negative;
        if self.grouping && format.group().is_none() {
            return Err(EvalError::UnsupportedExpression);
        }
        Ok(Display {
            pattern: self,
            magnitude,
            format,
            exponent,
        })
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

#[cfg(test)]
mod interleaved_tests;
#[cfg(test)]
mod tests;
