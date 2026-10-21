//! Charged fixed-point grouping and adornments without expanded scratch allocation.
use super::{
    super::{format_buffer::FormatBuffer, output::TextOutput},
    Kind, NumberDisplay,
};
use crate::{EvalError, functions::conversions::text::NumberFormat};
use std::fmt::Write;

const GROUP_WIDTH: usize = 3;
const ZERO: &str = "0";

pub(super) fn write(
    display: &NumberDisplay,
    output: &mut impl TextOutput,
) -> Result<(), EvalError> {
    let NumberDisplay {
        magnitude,
        kind,
        options,
        format,
    } = display;
    let mut coefficient = FormatBuffer::new();
    write!(coefficient, "{}", magnitude.coefficient).map_err(|_| EvalError::TextLimit)?;
    let digits = coefficient.text()?;
    if magnitude.negative {
        output.append(if options.parentheses { "(" } else { "-" })?;
    }
    let currency = if *kind == Kind::Currency {
        format.currency()
    } else {
        None
    };
    if currency == Some('$') {
        output.append("$")?;
    }
    let whole = if magnitude.coefficient == 0 {
        0
    } else {
        magnitude.point.max(0) as usize
    };
    if whole == 0 {
        if options.leading || options.places == 0 {
            output.append(ZERO)?;
        }
    } else {
        for index in 0..whole {
            if options.grouping && index > 0 && (whole - index).is_multiple_of(GROUP_WIDTH) {
                output.append(if *format == NumberFormat::DeDe {
                    "."
                } else {
                    ","
                })?;
            }
            output.append(digits.get(index..index + 1).unwrap_or(ZERO))?;
        }
    }
    if options.places > 0 {
        output.append(if *format == NumberFormat::DeDe {
            ","
        } else {
            "."
        })?;
        for fraction in 0..i32::from(options.places) {
            let index = magnitude.point + fraction;
            let digit = if magnitude.coefficient == 0 || index < 0 {
                ZERO
            } else {
                digits
                    .get(index as usize..index as usize + 1)
                    .unwrap_or(ZERO)
            };
            output.append(digit)?;
        }
    }
    if *kind == Kind::Percent {
        output.append("%")?;
    }
    if currency == Some('€') {
        output.append(" €")?;
    }
    if magnitude.negative && options.parentheses {
        output.append(")")?;
    }
    Ok(())
}
