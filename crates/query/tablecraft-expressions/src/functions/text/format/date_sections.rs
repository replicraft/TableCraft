//! Numeric serial sections with calendar rendering only for positive values.
use super::{
    lexer::{self, Kind, Tokens},
    sections::Sections,
};
use crate::{EvalError, functions::text::output::NumericBuffer};
use tablecraft_core::values::Value;

/// None selects the already parsed positive date mask. Other selected sections
/// must contain literals only, avoiding an inferred negative-date interpretation.
pub(super) fn literal_selection<'a>(
    value: &Value,
    sections: &Sections<'a>,
    scratch: &mut impl NumericBuffer,
) -> Result<Option<&'a str>, EvalError> {
    if !matches!(
        value,
        Value::Boolean(_)
            | Value::Byte(_)
            | Value::Int16(_)
            | Value::Int32(_)
            | Value::Int64(_)
            | Value::Single(_)
            | Value::Double(_)
            | Value::Currency(_)
            | Value::Decimal(_)
    ) {
        return Err(EvalError::UnsupportedExpression);
    }

    let (negative, zero) = super::sign(value)?;
    if !negative && !zero {
        return Ok(None);
    }

    let (section, _) = sections.numeric(negative, zero);
    let mut tokens = Tokens::new(section);
    while let Some(token) = tokens.next(&mut || scratch.step())? {
        match token.kind {
            Kind::Literal(_) | Kind::Symbol('%') => {}
            Kind::Symbol(symbol) if lexer::literal(symbol) => {}
            _ => return Err(EvalError::UnsupportedExpression),
        }
    }
    Ok(Some(section))
}

#[cfg(test)]
mod tests;
