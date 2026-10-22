//! Access Format masks, with borrowed section selection and bounded rendering.
mod lexer;
mod numeric;
mod sections;

use super::{
    TextContext, TextResult,
    casing::Casing,
    cstr, numeric_format,
    output::{NumericBuffer, TextOutput},
};
use crate::{
    EvalError,
    functions::{
        conversions::{Conversion, text::NumberFormat},
        dates::{FirstDayOfWeek, FirstWeekOfYear},
    },
    numeric::decimal::{FloatConversion, from_value},
};
use sections::Sections;
use tablecraft_core::values::Value;

/// Borrowed custom numeric/section recipe; scratch is private conversion storage.
#[derive(Debug, PartialEq, Eq)]
pub struct Format<'a> {
    value: &'a Value,
    mask: &'a str,
    number_format: Option<NumberFormat>,
}

/// Resolved format, independent of temporary numeric conversion storage.
pub struct Display<'a> {
    content: Content<'a>,
}

enum Content<'a> {
    Literal(&'a str),
    Word(&'static str),
    Numeric(numeric::Display<'a>),
}

pub(super) fn prepare<'a>(
    arguments: &'a [Value],
    context: TextContext<'_>,
) -> Result<TextResult<'a>, EvalError> {
    if !(1..=4).contains(&arguments.len()) {
        return Err(EvalError::UnsupportedExpression);
    }
    for (index, value) in arguments.iter().enumerate().skip(2) {
        let Value::Int32(option) = Conversion::Long.apply(value)? else {
            return Err(EvalError::TypeMismatch);
        };
        let maximum = if index == 2 {
            i32::from(FirstDayOfWeek::MAX)
        } else {
            i32::from(FirstWeekOfYear::MAX)
        };
        if !(0..=maximum).contains(&option) {
            return Err(EvalError::Domain);
        }
        if option == 0
            && (index == 2 && context.first_day_of_week.is_none()
                || index == 3 && context.first_week_of_year.is_none())
        {
            return Err(EvalError::CalendarPolicyRequired);
        }
    }
    let value = &arguments[0];
    let mask = match arguments.get(1) {
        None => "",
        Some(Value::Text(mask)) => mask,
        Some(Value::Null) => return Err(EvalError::InvalidNullConversion),
        Some(_) => return Err(EvalError::TypeMismatch),
    };
    if mask.is_empty() {
        return if matches!(value, Value::Null) {
            Ok(TextResult::Text(""))
        } else {
            cstr::prepare(value, context)
        };
    }
    if matches!(mask, "<" | ">") && !matches!(value, Value::Null) {
        let Value::Text(text) = value else {
            return Err(EvalError::TypeMismatch);
        };
        return Ok(TextResult::Casing(Casing::new(text, mask == ">")));
    }
    if mask.eq_ignore_ascii_case("General Number") && !matches!(value, Value::Null) {
        return numeric_format::prepare_cstr(
            value,
            context
                .number_format
                .ok_or(EvalError::NumberFormatRequired)?,
        );
    }
    Ok(TextResult::Format(Format {
        value,
        mask,
        number_format: context.number_format,
    }))
}

impl<'a> Format<'a> {
    /// Validate sections and resolve exact numeric/finite floating display parts.
    /// This does not allocate output or retain the caller's conversion scratch.
    pub fn resolve(&self, scratch: &mut impl NumericBuffer) -> Result<Display<'a>, EvalError> {
        scratch.step()?;
        if !scratch.text().is_empty() {
            return Err(EvalError::Domain);
        }
        let mask = if self.mask.eq_ignore_ascii_case("Fixed") {
            "0.00"
        } else if self.mask.eq_ignore_ascii_case("Standard") {
            "#,##0.00"
        } else if self.mask.eq_ignore_ascii_case("Percent") {
            "0.00%"
        } else if self.mask.eq_ignore_ascii_case("Currency") {
            match self.number_format.ok_or(EvalError::NumberFormatRequired)? {
                NumberFormat::EnUs => "$#,##0.00;($#,##0.00)",
                NumberFormat::DeDe => "#,##0.00 \"€\";-#,##0.00 \"€\"",
                _ => return Err(EvalError::UnsupportedExpression),
            }
        } else {
            self.mask
        };
        let sections = Sections::parse(mask, &mut || scratch.step())?;
        if matches!(self.value, Value::Null) {
            return Ok(Display {
                content: Content::Literal(sections.null()),
            });
        }
        let parsed;
        let value = if let Value::Text(text) = self.value {
            parsed = Conversion::Double.apply_text(
                text,
                self.number_format.ok_or(EvalError::NumberFormatRequired)?,
                scratch,
            )?;
            &parsed
        } else {
            self.value
        };
        let (negative, zero) = sign(value)?;
        for (name, yes, no) in [
            ("Yes/No", "Yes", "No"),
            ("True/False", "True", "False"),
            ("On/Off", "On", "Off"),
        ] {
            if mask.eq_ignore_ascii_case(name) {
                return Ok(Display {
                    content: Content::Word(if zero { no } else { yes }),
                });
            }
        }
        let (section, automatic_negative) = sections.numeric(negative, zero);
        let Some(pattern) = numeric::Pattern::parse(section, &mut || scratch.step())? else {
            return Ok(Display {
                content: Content::Literal(section),
            });
        };
        let format = self.number_format.ok_or(EvalError::NumberFormatRequired)?;
        Ok(Display {
            content: Content::Numeric(pattern.resolve(
                value,
                automatic_negative,
                format,
                scratch,
            )?),
        })
    }
}

impl Display<'_> {
    /// Emit selected output through the caller's metered fallible text sink.
    pub fn write(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        match &self.content {
            Content::Literal(mask) => numeric::literals(mask, output),
            Content::Word(text) => output.append(text),
            Content::Numeric(display) => display.write(output),
        }
    }
}

fn sign(value: &Value) -> Result<(bool, bool), EvalError> {
    let float = match value {
        Value::Single(value) => Some(f64::from(value.to_float())),
        Value::Double(value) => Some(value.to_float()),
        _ => None,
    };
    if let Some(value) = float {
        if !value.is_finite() {
            return Err(EvalError::NonFinite);
        }
        return Ok((value < 0.0, value == 0.0));
    }
    let value = from_value(value, FloatConversion::Retained)?;
    Ok((value.is_sign_negative(), value.is_zero()))
}

#[cfg(test)]
mod tests;
