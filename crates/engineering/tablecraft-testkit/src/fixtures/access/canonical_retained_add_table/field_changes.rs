//! Independent retained field RENAME/DROP images preserve row and property history.
use crate::fixtures::access::{
    put16, put32, raw_catalog_properties as properties, raw_property_groups, raw_schema_drop,
};

const PAGE: usize = 4096;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

pub fn rename(
    before: &[u8],
    definition: u32,
    catalog_root: usize,
    old: &str,
    new: &str,
) -> Vec<u8> {
    assert!(old.is_ascii() && new.is_ascii() && !new.is_empty() && new.len() <= 64);
    let page = definition as usize * PAGE;
    let active = word(before, page + 45);
    let mut position = page + 75 + 25 * active;
    let encoded = old
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let replacement = new
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut found = None;
    for _ in 0..active {
        let bytes = word(before, position);
        if before[position + 2..position + 2 + bytes] == encoded {
            assert!(found.replace(position + 2..position + 2 + bytes).is_none());
        }
        position += 2 + bytes;
    }
    let selected = found.unwrap();
    if old == new {
        return before.to_vec();
    }
    let length = u32::from_le_bytes(before[page + 8..page + 12].try_into().unwrap()) as usize;
    let delta = replacement.len() as isize - encoded.len() as isize;
    let new_length = length.checked_add_signed(delta).unwrap();
    assert!(new_length <= PAGE - 8);
    let mut after = before.to_vec();
    after[selected.start..selected.start + replacement.len()].copy_from_slice(&replacement);
    after[selected.start + replacement.len()..page + new_length]
        .copy_from_slice(&before[selected.end..page + length]);
    put16(&mut after, selected.start - 2, replacement.len() as u16);
    put32(&mut after, page + 8, new_length as u32);
    put16(
        &mut after,
        page + 2,
        word(before, page + 2).checked_add_signed(-delta).unwrap() as u16,
    );
    let body = raw_property_groups::rename(&properties::body(before, catalog_root), old, new);
    let candidate = properties::candidate(before, body.len());
    properties::append(before, after, catalog_root, &body, candidate)
}

pub fn drop_tail(before: &[u8], definition: u32, catalog_root: usize, name: &str) -> Vec<u8> {
    let page = definition as usize * PAGE;
    let active = word(before, page + 45);
    let mut last = page + 75 + 25 * active;
    for _ in 1..active {
        last += 2 + word(before, last);
    }
    let expected = name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(&before[last + 2..last + 2 + word(before, last)], expected);
    let declaration = raw_schema_drop::definition(
        &before[page..page + PAGE],
        name,
        raw_schema_drop::Slack::Preserve,
    );
    let mut after = before.to_vec();
    after[page..page + PAGE].copy_from_slice(&declaration);
    let body = raw_property_groups::without_group(&properties::body(before, catalog_root), name);
    let candidate = properties::candidate(before, body.len());
    properties::append(before, after, catalog_root, &body, candidate)
}

/// Retained history with a newly declared tail, optional old-row materialization
/// and a fresh generated row. The extra page is actual owned/hinted storage.
pub fn source(
    case: super::fields::Case,
    target: super::fields::Target,
    filled: bool,
    inserted: bool,
    extra_page: bool,
) -> super::fields::Source {
    let mut source = super::fields::source(case);
    if extra_page {
        let roots = std::mem::take(&mut source.roots);
        let extended = super::fields::property_pages::extend_source(
            source,
            super::fields::property_pages::Case::OriginalInsufficient,
        );
        source = super::fields::Source {
            bytes: extended.bytes,
            definition: extended.definition,
            catalog_root: extended.catalog_root,
            roots,
        };
    }
    source.bytes = super::fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        target,
    );
    if filled {
        let value = match target {
            super::fields::Target::Number => tablecraft_core::values::Value::Int32(123),
            super::fields::Target::ShortText => {
                tablecraft_core::values::Value::Text("日本語🦀".into())
            }
        };
        source.bytes = super::fields::fill(
            &source.bytes,
            source.definition,
            source.roots[0],
            target,
            Some(value),
        );
    }
    if inserted {
        let root = word(&source.bytes, (source.definition as usize + 1) * PAGE + 12);
        source.bytes = super::fields::append(
            &source.bytes,
            source.definition,
            target,
            super::rows::Input::Omitted,
        );
        source.roots.push(root);
    }
    source
}
