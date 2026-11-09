//! Independent fitting ADD after an explicit database OFF body.
//! Source and AFTER pins come from the maintained first-party SDK experiment;
//! no production parser, encoder or native mutation participates in this oracle.
#[cfg(test)]
mod tests;

use super::{Source, SourceCase as MixedSource, properties, repeated, storage};
use crate::fixtures::access::canonical_database_options;

/// The two complete nine-role SDK BEFORE sources in the retained observation.
#[derive(Clone, Copy, Debug)]
pub enum SourceCase {
    /// Generated ID + Score, followed by the database OFF role.
    Number,
    /// Generated ID + Label + AfterLabel + OtherLabel, then database OFF.
    Mixed,
}

/// Build the exact independent nine-role BEFORE image for this observed class.
pub fn source(case: SourceCase) -> Vec<u8> {
    let fields = match case {
        SourceCase::Number => repeated::source(Source::IdOnly),
        SourceCase::Mixed => {
            let label = super::append_text(&super::source(MixedSource::IdOnly), "Label", 255, true);
            let number = super::append_number(&label, "AfterLabel");
            super::append_text(&number, "OtherLabel", 255, true)
        }
    };
    canonical_database_options::append_explicit_off_to_owned_page(&fields)
}

/// Fixed nullable LONG with stored Required(false) and literal zero default.
pub fn append_number(before: &[u8], name: &str) -> Vec<u8> {
    let after = super::append_definition(before, name);
    let original_body = storage::selected_body(before);
    let body = properties::append_number(&original_body, name);
    storage::append(before, after, &body)
}

/// Fixed Text255, Unicode compression, Required(false), AllowZeroLength(true)
/// and absent default; these are the exact observed target declarations.
pub fn append_text(before: &[u8], name: &str) -> Vec<u8> {
    let after = super::append_text_definition(before, name, 255, true);
    let original_body = storage::selected_body(before);
    let body = properties::append_text(&original_body, name);
    storage::append(before, after, &body)
}

pub mod history;
