//! Exact XML 1.0 Fifth Edition names, without normalization or Access decoding.
use super::TemplateError;
use crate::ReadError;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn ncname(value: &str) -> bool {
    let mut characters = value.chars();
    characters.next().is_some_and(start) && characters.all(continuation)
}

pub(super) fn qname(value: &str, budget: &mut ResourceBudget) -> Result<(), TemplateError> {
    budget
        .charge(Resource::Steps, value.len() as u64)
        .map_err(ReadError::from)?;

    let valid = match value.split_once(':') {
        Some((prefix, local)) => ncname(prefix) && ncname(local),
        None => ncname(value),
    };
    if !valid {
        return Err(TemplateError::Malformed);
    }
    Ok(())
}

fn start(character: char) -> bool {
    matches!(character,
        'A'..='Z' | '_' | 'a'..='z'
        | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}'
        | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}'
        | '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}'
        | '\u{2c00}'..='\u{2fef}' | '\u{3001}'..='\u{d7ff}'
        | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}'
        | '\u{10000}'..='\u{effff}'
    )
}

fn continuation(character: char) -> bool {
    start(character)
        || matches!(character,
            '-' | '.' | '0'..='9' | '\u{b7}'
            | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'
        )
}

#[cfg(test)]
mod tests;
