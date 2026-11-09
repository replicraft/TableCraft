use crate::{InspectError, header::check};
use tablecraft_core::limits::ResourceBudget;

/// XML 1.0 fifth-edition NCName characters. A QName has one optional prefix;
/// namespace binding is checked separately by the namespace-aware reader.
pub(super) fn qualified(
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let mut parts = name.split(':');
    for _ in 0..2 {
        let Some(part) = parts.next() else {
            return Ok(());
        };
        let mut chars = part.chars();
        if !chars.next().is_some_and(start) {
            return Err(InspectError::Malformed);
        }
        for scalar in chars {
            check(budget, cancelled)?;
            if !start(scalar)
                && !matches!(scalar, '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}')
            {
                return Err(InspectError::Malformed);
            }
        }
    }
    if parts.next().is_some() {
        return Err(InspectError::Malformed);
    }
    Ok(())
}

fn start(scalar: char) -> bool {
    matches!(scalar,
        'A'..='Z' | '_' | 'a'..='z'
        | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}'
        | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}'
        | '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}'
        | '\u{2c00}'..='\u{2fef}' | '\u{3001}'..='\u{d7ff}'
        | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}'
        | '\u{10000}'..='\u{effff}'
    )
}
