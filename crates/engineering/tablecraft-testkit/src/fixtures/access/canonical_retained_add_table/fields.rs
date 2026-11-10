//! Independent nullable field ADD at actual retained native definitions/catalog roots.
pub mod property_pages;
use crate::fixtures::access::{
    canonical_created_number::mixed::properties,
    canonical_table_drop::retained as dropped,
    generated_record_insert::{
        placement::{self, Pages},
        record,
    },
    raw_forwarding_chain, raw_schema_add,
};

use tablecraft_core::values::Value;

const PAGE: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub enum Target {
    Number,
    ShortText,
}

impl Target {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Number => "NextNumber",
            Self::ShortText => "NextText",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Case {
    FirstNumber,
    SecondText,
    OriginalSparse,
}

pub const CASES: [Case; 3] = [Case::FirstNumber, Case::SecondText, Case::OriginalSparse];

pub struct Source {
    pub bytes: Vec<u8>,
    pub definition: u32,
    pub catalog_root: usize,
    pub roots: Vec<usize>,
}

pub fn source(case: Case) -> Source {
    let original = dropped::source(match case {
        Case::FirstNumber => dropped::Case::FirstNumber,
        Case::SecondText => dropped::Case::SecondText,
        Case::OriginalSparse => dropped::Case::OriginalSparse,
    });
    Source {
        bytes: original.bytes,
        definition: original.definition,
        catalog_root: original.catalog_root,
        roots: original.roots,
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// Old records remain byte-identical and the new logical native field is NULL.
/// A new complete MR2 snapshot is appended; old inline/body bytes remain history.
pub fn after(before: &[u8], definition: u32, catalog_root: usize, target: Target) -> Vec<u8> {
    after_with_page(before, definition, catalog_root, target, None)
}

fn after_on_property_page(
    before: &[u8],
    definition: u32,
    catalog_root: usize,
    target: Target,
    property_page: u32,
) -> Vec<u8> {
    after_with_page(
        before,
        definition,
        catalog_root,
        target,
        Some(property_page),
    )
}

fn after_with_page(
    before: &[u8],
    definition: u32,
    catalog_root: usize,
    target: Target,
    selected_page: Option<u32>,
) -> Vec<u8> {
    let row = raw_forwarding_chain::record(before, 4, catalog_root);
    assert_eq!(
        u32::from_le_bytes(row[2..6].try_into().unwrap()),
        definition
    );
    let body = crate::fixtures::access::raw_catalog_properties::body(before, catalog_root);
    let (kind, body) = match target {
        Target::Number => (
            raw_schema_add::Kind::Number,
            properties::append_number(&body, target.name()),
        ),
        Target::ShortText => (
            raw_schema_add::Kind::ShortText,
            properties::append_text(&body, target.name()),
        ),
    };
    let property_page = selected_page.unwrap_or_else(|| {
        crate::fixtures::access::raw_catalog_properties::candidate(before, body.len())
    });
    let after = raw_schema_add::append(before, definition, target.name(), kind);
    crate::fixtures::access::raw_catalog_properties::append(
        before,
        after,
        catalog_root,
        &body,
        property_page,
    )
}

/// Insert with the qualified literal sibling defaults, preserving original
/// Omitted/NULL/supplied outcomes for the newly added last declaration.
pub fn append(
    before: &[u8],
    definition: u32,
    target: Target,
    input: super::rows::Input<'_>,
) -> Vec<u8> {
    let page = definition as usize * PAGE;
    let active = word(before, page + 45);
    let selected = word(before, page + 75 + 25 * (active - 1) + 5);
    let mut value = Some(match (target, input) {
        (Target::Number, super::rows::Input::Omitted) => Value::Int32(0),
        (_, super::rows::Input::Omitted | super::rows::Input::Null) => Value::Null,
        (Target::Number, super::rows::Input::Number(value)) => Value::Int32(value),
        (Target::ShortText, super::rows::Input::Text(value)) => Value::Text(value.into()),
        _ => panic!("literal input must match the selected declaration"),
    });
    let mut values = Vec::new();
    for field in 1..active {
        let at = page + 75 + 25 * field;
        let native = word(before, at + 5);
        values.push((
            native,
            if native == selected {
                value.take().unwrap()
            } else if before[at] == 4 {
                Value::Int32(0)
            } else {
                Value::Null
            },
        ));
    }
    let pages = Pages {
        definition: definition as usize,
        data: definition as usize + 1,
        index: definition as usize + 2,
    };
    let id = placement::next_id(before, pages);
    let row = record::build(before, definition as usize, id, &values);
    placement::append(before, pages, id, &row)
}

/// Materialize or replace the last newly added field, preserving raw sibling
/// segments, every retained footer slot and old logical NULL presence.
pub fn fill(
    before: &[u8],
    definition: u32,
    root: usize,
    target: Target,
    value: Option<Value>,
) -> Vec<u8> {
    let old = raw_forwarding_chain::record(before, definition + 1, root);
    let declaration = &before[definition as usize * PAGE..(definition as usize + 1) * PAGE];
    let count = word(old, 0);
    let active = word(declaration, 45);
    let has_variables = (0..active - 1).any(|index| {
        let at = 75 + index * 25;
        declaration[at + 15] & 1 == 0 && word(declaration, at + 5) < count
    }) || count == word(declaration, 41) && word(declaration, 43) != 0;
    let (kind, payload) = match (target, value) {
        (Target::Number, Some(Value::Int32(value))) => (
            raw_schema_add::Kind::Number,
            Some(value.to_le_bytes().to_vec()),
        ),
        (Target::Number, None) => (raw_schema_add::Kind::Number, None),
        (Target::ShortText, Some(Value::Text(value))) => {
            let units = value.encode_utf16().collect::<Vec<_>>();
            assert!(units.len() <= 255);
            let bytes = if units.len() > 2 && units.iter().all(|unit| (1..=255).contains(unit)) {
                let mut bytes = vec![255, 254];
                bytes.extend(units.into_iter().map(|unit| unit as u8));
                bytes
            } else {
                units.into_iter().flat_map(u16::to_le_bytes).collect()
            };
            (raw_schema_add::Kind::ShortText, Some(bytes))
        }
        (Target::ShortText, None) => (raw_schema_add::Kind::ShortText, None),
        _ => panic!("literal replacement must match the new field"),
    };
    let row = crate::fixtures::access::raw_historical_row::reconstruct(
        old,
        declaration,
        has_variables,
        kind,
        payload.as_deref(),
    );
    raw_forwarding_chain::place_terminal(before, definition + 1, root, &row)
}
