//! Borrowed hyperlink-field components; never follow or normalize an address.
use super::TextResult;
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;

const COMPONENT_COUNT: usize = 4;
const DELIMITER_COUNT: usize = COMPONENT_COUNT - 1;
const DELIMITER: char = '#';

/// Access AcHyperlinkPart selectors; SQL requires an explicit numeric argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum HyperlinkPart {
    /// Display text, falling back to the first nonempty address/subaddress.
    DisplayedValue = 0,
    /// Only the explicitly stored display text.
    DisplayText = 1,
    /// External address without its subaddress.
    Address = 2,
    /// Stored subaddress.
    SubAddress = 3,
    /// Stored tooltip text.
    ScreenTip = 4,
    /// Address with a hash-delimited nonempty subaddress.
    FullAddress = 5,
}

impl TryFrom<i32> for HyperlinkPart {
    type Error = EvalError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::DisplayedValue),
            1 => Ok(Self::DisplayText),
            2 => Ok(Self::Address),
            3 => Ok(Self::SubAddress),
            4 => Ok(Self::ScreenTip),
            5 => Ok(Self::FullAddress),
            _ => Err(EvalError::Domain),
        }
    }
}

/// Extract a borrowed component, checking work/cancellation while locating the
/// first three delimiters. Remaining hashes belong to the tooltip in this profile.
pub fn extract(
    text: &str,
    part: HyperlinkPart,
    mut step: impl FnMut() -> Result<(), EvalError>,
) -> Result<&str, EvalError> {
    step()?;
    let mut fields = [""; COMPONENT_COUNT];
    let mut separators = [None; DELIMITER_COUNT];
    let mut field = 0;
    let mut start = 0;
    for (position, character) in text.char_indices() {
        step()?;
        if character == DELIMITER {
            fields[field] = &text[start..position];
            separators[field] = Some(position);
            field += 1;
            start = position + DELIMITER.len_utf8();
            if field == DELIMITER_COUNT {
                break;
            }
        }
    }
    fields[field] = &text[start..];
    let [display, address, subaddress, screentip] = fields;

    Ok(match part {
        HyperlinkPart::DisplayedValue => [display, address, subaddress]
            .into_iter()
            .find(|part| !part.is_empty())
            .unwrap_or(""),
        HyperlinkPart::DisplayText => display,
        HyperlinkPart::Address => address,
        HyperlinkPart::SubAddress => subaddress,
        HyperlinkPart::ScreenTip => screentip,
        HyperlinkPart::FullAddress if subaddress.is_empty() => address,
        HyperlinkPart::FullAddress => {
            let start = separators
                .first()
                .copied()
                .flatten()
                .ok_or(EvalError::TypeMismatch)?
                + DELIMITER.len_utf8();
            let end = separators.last().copied().flatten().unwrap_or(text.len());
            &text[start..end]
        }
    })
}

pub(super) fn apply<'a>(
    arguments: &'a [Value],
    step: impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'a>, EvalError> {
    let [link, selector] = arguments else {
        return Err(EvalError::UnsupportedExpression);
    };
    let Value::Int32(selector) = Conversion::Long.apply(selector)? else {
        return Err(EvalError::TypeMismatch);
    };
    apply_part(|| Ok(link), selector, step)
}

pub(crate) fn apply_part<'a>(
    link: impl FnOnce() -> Result<&'a Value, EvalError>,
    selector: i32,
    step: impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'a>, EvalError> {
    let part = HyperlinkPart::try_from(selector)?;
    match link()? {
        Value::Null => Ok(TextResult::Null),
        Value::Text(text) => extract(text, part, step).map(TextResult::Text),
        _ => Err(EvalError::TypeMismatch),
    }
}

#[cfg(test)]
mod tests;
