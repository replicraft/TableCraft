//! Access Format masks, with borrowed section selection and bounded rendering.
mod date;
mod lexer;
mod numeric;
mod sections;
mod text;

use super::{
    TextContext, TextResult,
    casing::{CaseLocale, Casing},
    cstr, numeric_format,
    output::{NumericBuffer, TextOutput},
};
use crate::{
    EvalError,
    functions::{
        conversions::{Conversion, text::NumberFormat},
        dates::{
            TwoDigitYearWindow, WeekContext, WeekSettings,
            parsing::{DateParseContext, DateTextFormat, ReferenceYear},
        },
    },
    numeric::decimal::{FloatConversion, from_value},
};
use sections::Sections;
use tablecraft_core::values::Value;

/// Borrowed custom numeric/date/section recipe; scratch is private conversion storage.
#[derive(Debug, PartialEq, Eq)]
pub struct Format<'a> {
    value: &'a Value,
    mask: &'a str,
    number_format: Option<NumberFormat>,
    date_format: Option<DateTextFormat>,
    year_window: TwoDigitYearWindow,
    reference_year: Option<ReferenceYear>,
    weeks: WeekSettings,
}

/// Resolved format, independent of temporary numeric conversion storage.
pub struct Display<'a> {
    content: Content<'a>,
}

enum Content<'a> {
    Literal(&'a str),
    Word(&'static str),
    Numeric(numeric::Display<'a>),
    Date(date::Display<'a>),
    Text(text::Display<'a>),
}

pub(super) fn prepare<'a>(
    arguments: &'a [Value],
    context: TextContext<'_>,
) -> Result<TextResult<'a>, EvalError> {
    if !(1..=4).contains(&arguments.len()) {
        return Err(EvalError::UnsupportedExpression);
    }
    for value in arguments.iter().skip(2) {
        if matches!(value, Value::Null) {
            return Err(EvalError::InvalidNullConversion);
        }
    }
    let weeks = WeekSettings::parse(
        arguments.get(2),
        arguments.get(3),
        WeekContext {
            first_day: context.first_day_of_week,
            first_week: context.first_week_of_year,
        },
    )?;

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
        date_format: context.date_format,
        year_window: context.year_window,
        reference_year: context.reference_year,
        weeks,
    }))
}

impl<'a> Format<'a> {
    /// Validate sections and resolve numeric display or Gregorian calendar parts.
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
        } else if self.mask.eq_ignore_ascii_case("Scientific") {
            "0.00E+00"
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
        if text::Pattern::recognizes(sections.first(), &mut || scratch.step())? {
            let source = match self.value {
                Value::Text(source) => source.as_str(),
                Value::Null => "",
                _ => return Err(EvalError::TypeMismatch),
            };
            let section = sections.text(source.is_empty())?;
            return Ok(Display {
                content: Content::Text(text::Display::new(source, section, scratch)?),
            });
        }
        if matches!(self.value, Value::Null) {
            return Ok(Display {
                content: Content::Literal(sections.null()),
            });
        }
        let boolean_style = ["Yes/No", "True/False", "On/Off"]
            .iter()
            .any(|name| mask.eq_ignore_ascii_case(name));
        if !boolean_style
            && let Some(pattern) = date::Pattern::parse(sections.first(), &mut || scratch.step())?
        {
            if !sections.is_single() {
                return Err(EvalError::UnsupportedExpression);
            }
            let display = pattern.resolve(
                self.value,
                DateParseContext {
                    date_format: self.date_format,
                    year_window: self.year_window,
                    reference_year: self.reference_year,
                    number_format: self.number_format,
                },
                self.weeks,
                scratch,
            )?;
            return Ok(Display {
                content: Content::Date(display),
            });
        }

        let parsed;
        let value = match self.value {
            Value::Text(text) => {
                parsed = Conversion::Double.apply_text(
                    text,
                    self.number_format.ok_or(EvalError::NumberFormatRequired)?,
                    scratch,
                )?;
                &parsed
            }
            Value::DateTime(_) => {
                // Numeric masks display the stored OLE serial, not the rounded
                // calendar or its equivalent positive fractional representation.
                parsed = Conversion::Double.apply(self.value)?;
                &parsed
            }
            value => value,
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
        self.write_with_case_locale(None, output)
    }

    /// Render text casing with an explicit host language; other masks ignore it.
    pub fn write_with_case_locale(
        &self,
        locale: Option<&CaseLocale>,
        output: &mut impl TextOutput,
    ) -> Result<(), EvalError> {
        output.step()?;
        match &self.content {
            Content::Literal(mask) => numeric::literals(mask, output),
            Content::Word(text) => output.append(text),
            Content::Numeric(display) => display.write(output),
            Content::Date(display) => display.write(output),
            Content::Text(display) => display.write(locale, output),
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
