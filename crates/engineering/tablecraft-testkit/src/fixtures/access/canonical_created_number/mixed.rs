//! Independent mixed field AFTERs from the maintained Short Text literal model.
//! No engine/SDK serializer, parser or inspection API participates in these bytes.
pub mod after_off;
pub(in crate::fixtures::access) mod properties;
pub(in crate::fixtures::access) mod storage;
#[cfg(test)]
mod tests;

use super::{Source, repeated, source as number_source, word};
use crate::fixtures::access::{canonical_created_add_column, put16};

const PAGE: usize = 4096;
const DEFINITION_PAGE: usize = 7;
const FIELD_COUNT_OFFSET: usize = 45;
const VARIABLE_COUNT_OFFSET: usize = 43;
const FIELD_START: usize = 75;
const FIELD_BYTES: usize = 25;
const TYPE_OFFSET: usize = 0;
const SORT_OFFSET: usize = 11;
const FLAGS_OFFSET: usize = 15;
const EXTENDED_FLAGS_OFFSET: usize = 16;
const FIXED_OFFSET: usize = 21;
const WIDTH_OFFSET: usize = 23;
const MAX_TEXT_UNITS: u16 = 255;
const TEXT_TYPE: u8 = 10;
const VARIABLE_TEXT_FLAGS: u8 = 2;
const GENERAL_SORT_ORDER: [u8; 4] = [9, 4, 0, 1];

/// Exact four maintained SDK BEFORE classes; names are independent raw inputs.
#[derive(Clone, Copy, Debug)]
pub enum SourceCase {
    /// Initial generated-primary table with no field property bag.
    IdOnly,
    /// Existing Score Required(false)/zero stored in an inline bag.
    InlineNumber,
    /// External history from the exact Score then Score2 SDK source.
    TwoNumberHistory,
    /// The preceding history with its exact third field named Third.
    ThreeNumberHistory,
}

/// Reuse the independently constructed canonical Number source owners.
pub fn source(case: SourceCase) -> Vec<u8> {
    match case {
        SourceCase::IdOnly => number_source(Source::IdOnly),
        SourceCase::InlineNumber => number_source(Source::GuiRequiredZero),
        SourceCase::TwoNumberHistory => {
            repeated::append(&repeated::source(Source::IdOnly), "Score2")
        }
        SourceCase::ThreeNumberHistory => {
            repeated::append(&source(SourceCase::TwoNumberHistory), "Third")
        }
    }
}

/// Append TEXT with explicit stored Required(false), AllowZeroLength(true),
/// absent DefaultValue, chosen UTF16 width and Unicode compression bit.
pub fn append_text(before: &[u8], name: &str, units: u16, compressed: bool) -> Vec<u8> {
    let after = append_text_definition(before, name, units, compressed);
    let original_body = storage::body(before);
    let body = properties::append_text(&original_body, name);
    storage::append(before, after, &body)
}

/// Append nullable LONG with stored Required(false) and a literal zero default,
/// retaining all previous Text descriptors, flags, ordinals and property groups.
pub fn append_number(before: &[u8], name: &str) -> Vec<u8> {
    let after = append_definition(before, name);
    let original_body = storage::body(before);
    let body = properties::append_number(&original_body, name);
    storage::append(before, after, &body)
}

fn append_text_definition(before: &[u8], name: &str, units: u16, compressed: bool) -> Vec<u8> {
    assert!((1..=MAX_TEXT_UNITS).contains(&units));
    let original = &before[DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE];
    let mut after = append_definition(before, name);
    let definition = &mut after[DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE];
    let field = FIELD_START + FIELD_BYTES * word(original, FIELD_COUNT_OFFSET);
    definition[field + TYPE_OFFSET] = TEXT_TYPE;
    definition[field + SORT_OFFSET..field + FLAGS_OFFSET].copy_from_slice(&GENERAL_SORT_ORDER);
    definition[field + FLAGS_OFFSET] = VARIABLE_TEXT_FLAGS;
    definition[field + EXTENDED_FLAGS_OFFSET] = u8::from(compressed);
    put16(definition, field + FIXED_OFFSET, 0);
    put16(definition, field + WIDTH_OFFSET, units * 2);
    put16(
        definition,
        VARIABLE_COUNT_OFFSET,
        (word(original, VARIABLE_COUNT_OFFSET) + 1) as u16,
    );
    after
}

fn append_definition(before: &[u8], name: &str) -> Vec<u8> {
    assert!(before.len() == 10 * PAGE || before.len() == 11 * PAGE);
    // The actual raw TD owner copies EVERY old field descriptor verbatim, then
    // relocates names/index tail and frames one LONG descriptor. In particular,
    // old Text compression bits and existing variable ordinals are not rebuilt.
    let rebuilt = canonical_created_add_column::append(&before[..10 * PAGE], name);
    let mut after = before.to_vec();
    after[DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE]
        .copy_from_slice(&rebuilt[DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE]);
    after
}
