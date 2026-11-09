//! Independent fitted TDEF/MR2/LVAL images with fixed source pages and row slots.
use crate::fixtures::access::{
    canonical_created_number::{Source, mixed, repeated},
    canonical_database_options, put16, put32,
};
use std::ops::Range;

const PAGE: usize = 4096;
const CATALOG: usize = 4;
const DEFINITION: usize = 7;
const PROPERTY: usize = 10;
const DIRECTORY: usize = 14;
const ROW_COUNT: usize = 12;
const FREE: usize = 2;
const DEFINITION_LENGTH: usize = 8;
const FIELD_COUNT: usize = 45;
const FIELD_START: usize = 75;
const FIELD_BYTES: usize = 25;
const OFFSET_MASK: usize = 0x1fff;
const FORWARD: usize = 0x4000;
const TARGET: usize = 0x8000;
const SINGLE: u32 = 0x4000_0000;
const LENGTH_MASK: u32 = 0x3fff_ffff;
const ROOT_SLOT: usize = 7;
const FOOTER_BYTES: usize = 5;
const PROPERTY_START_FROM_FOOTER: usize = 18;
const PROPERTY_END_FROM_FOOTER: usize = 20;
const DESCRIPTOR_BYTES: usize = 12;
const GROUP_NAME_PREFIX: usize = 12;
const NAME_LIMIT: usize = 64;

/// The same four independently constructed settled/OFF originals used by the
/// retained same-width observation. This introduces no additional native source.
#[derive(Clone, Copy, Debug)]
pub enum SourceCase {
    NumberFirst,
    NumberHistory,
    InlineNumberThenText,
    TextNumberText,
}

pub fn old_name(case: SourceCase) -> &'static str {
    match case {
        SourceCase::TextNumberText => "Label",
        _ => "Score",
    }
}

pub fn source(case: SourceCase) -> Vec<u8> {
    let fields = match case {
        SourceCase::NumberFirst => repeated::source(Source::IdOnly),
        SourceCase::NumberHistory => repeated::append(&repeated::source(Source::IdOnly), "Score2"),
        SourceCase::InlineNumberThenText => mixed::append_text(
            &mixed::source(mixed::SourceCase::InlineNumber),
            "Label",
            255,
            true,
        ),
        SourceCase::TextNumberText => {
            let text = mixed::append_text(
                &mixed::source(mixed::SourceCase::IdOnly),
                "Label",
                255,
                true,
            );
            let number = mixed::append_number(&text, "AfterLabel");
            mixed::append_text(&number, "OtherLabel", 255, true)
        }
    };
    canonical_database_options::append_explicit_off_to_owned_page(&fields)
}

/// Complete fixed-length pair. Only unequal-width ASCII proposals are modeled;
/// identity, data/index pages, retired histories and database OFF payload survive.
pub fn pair(case: SourceCase, new_name: &str) -> [Vec<u8>; 2] {
    let before = source(case);
    let after = rename(&before, old_name(case), new_name);
    [before, after]
}

/// Natural retained Number history with only 46 bytes left after explicit OFF.
/// The twenty-character second field appears in eleven retained property bodies;
/// no padding or page bytes are forged to manufacture the capacity boundary.
pub fn near_capacity_source() -> Vec<u8> {
    let first = repeated::source(Source::IdOnly);
    let mut fields = repeated::append(&first, "SecondNumberColumnXX");
    for number in 3..=12 {
        fields = repeated::append(&fields, &format!("N{number}"));
    }
    let before = canonical_database_options::append_explicit_off_to_owned_page(&fields);
    assert_eq!(word(&before, PROPERTY * PAGE + FREE), 46);
    before
}

/// A complete independent AFTER only when the requested unequal ASCII name fits
/// the actual existing row page. Score's five units can grow to 28, but not 29.
pub fn near_capacity_pair(new_name: &str) -> [Vec<u8>; 2] {
    let before = near_capacity_source();
    let after = rename(&before, "Score", new_name);
    [before, after]
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn shifted(offset: usize, delta: isize) -> usize {
    offset.checked_add_signed(delta).unwrap()
}

fn row(bytes: &[u8], page: usize, slot: usize) -> Range<usize> {
    let base = page * PAGE;
    assert!(slot < word(bytes, base + ROW_COUNT));
    let start = word(bytes, base + DIRECTORY + slot * 2) & OFFSET_MASK;
    let end = if slot == 0 {
        PAGE
    } else {
        word(bytes, base + DIRECTORY + (slot - 1) * 2) & OFFSET_MASK
    };
    assert!(DIRECTORY + word(bytes, base + ROW_COUNT) * 2 <= start && start < end && end <= PAGE);
    base + start..base + end
}

fn utf16(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for unit in text.encode_utf16() {
        bytes.extend(unit.to_le_bytes());
    }
    bytes
}

fn field_span(bytes: &[u8], old: &[u8]) -> Range<usize> {
    let base = DEFINITION * PAGE;
    let count = word(bytes, base + FIELD_COUNT);
    assert_eq!(count, word(bytes, base + 41));
    let mut position = base + FIELD_START + count * FIELD_BYTES;
    let mut selected = None;
    for index in 0..count {
        let length = word(bytes, position);
        let span = position + 2..position + 2 + length;
        if &bytes[span.start..span.end] == old {
            assert_ne!(index, 0);
            assert!(selected.replace(span.start..span.end).is_none());
        }
        position = span.end;
    }
    selected.unwrap()
}

fn catalog_descriptor(bytes: &[u8]) -> usize {
    let root = row(bytes, CATALOG, ROOT_SLOT);
    let slot = if word(bytes, CATALOG * PAGE + DIRECTORY + ROOT_SLOT * 2) & FORWARD != 0 {
        let reference = long(bytes, root.start);
        assert_eq!(reference >> 8, CATALOG as u32);
        let slot = (reference & 255) as usize;
        assert_ne!(
            word(bytes, CATALOG * PAGE + DIRECTORY + slot * 2) & TARGET,
            0
        );
        slot
    } else {
        ROOT_SLOT
    };
    let record = row(bytes, CATALOG, slot);
    let footer = record.end - FOOTER_BYTES;
    let start = word(bytes, footer - PROPERTY_START_FROM_FOOTER);
    let end = word(bytes, footer - PROPERTY_END_FROM_FOOTER);
    assert_eq!(end - start, DESCRIPTOR_BYTES);
    record.start + start
}

fn group_name(bytes: &[u8], body: &Range<usize>, old: &[u8]) -> Range<usize> {
    assert_eq!(&bytes[body.start..body.start + 4], b"MR2\0");
    let mut position = body.start + 4 + long(bytes, body.start + 4) as usize;
    let mut selected = None;
    while position < body.end {
        let length = long(bytes, position) as usize;
        let name_bytes = word(bytes, position + 10);
        assert_eq!(long(bytes, position + 6) as usize, name_bytes + 6);
        let name = position + GROUP_NAME_PREFIX..position + GROUP_NAME_PREFIX + name_bytes;
        assert!(name.end <= position + length && position + length <= body.end);
        if word(bytes, position + 4) == 1 && &bytes[name.start..name.end] == old {
            assert!(selected.replace(name).is_none());
        }
        position += length;
    }
    assert_eq!(position, body.end);
    selected.unwrap()
}

fn rename(before: &[u8], old_name: &str, new_name: &str) -> Vec<u8> {
    assert!(old_name.is_ascii() && new_name.is_ascii());
    assert!((1..=NAME_LIMIT).contains(&new_name.len()));
    assert_ne!(old_name.len(), new_name.len());
    assert_eq!(before.len(), 11 * PAGE);
    let old = utf16(old_name);
    let new = utf16(new_name);
    let delta = new.len() as isize - old.len() as isize;
    let name = field_span(before, &old);
    let descriptor = catalog_descriptor(before);
    let tagged = long(before, descriptor);
    let reference = long(before, descriptor + 4);
    assert_eq!(tagged & !LENGTH_MASK, SINGLE);
    assert_eq!(reference >> 8, PROPERTY as u32);
    assert_eq!(long(before, descriptor + 8), 0);
    let slot = (reference & 255) as usize;
    let rows = word(before, PROPERTY * PAGE + ROW_COUNT);
    assert_eq!(slot, rows - 2);
    let body = row(before, PROPERTY, slot);
    assert_eq!(body.len(), (tagged & LENGTH_MASK) as usize);
    let database = row(before, PROPERTY, rows - 1);
    assert_eq!(database.end, body.start);
    let property_name = group_name(before, &body, &old);
    let group = property_name.start - GROUP_NAME_PREFIX;

    let definition_base = DEFINITION * PAGE;
    let length = long(before, definition_base + DEFINITION_LENGTH) as usize;
    let end = definition_base + length;
    assert!(name.end <= end && shifted(length, delta) <= PAGE - 8);
    assert_eq!(word(before, definition_base + FREE), PAGE - length - 8);
    let property_base = PROPERTY * PAGE;
    assert_eq!(
        word(before, property_base + FREE),
        database.start - property_base - DIRECTORY - rows * 2
    );
    assert!(shifted(database.start, -delta) >= property_base + DIRECTORY + rows * 2);

    let mut renamed_body = Vec::new();
    renamed_body.extend(&before[body.start..property_name.start]);
    renamed_body.extend(&new);
    renamed_body.extend(&before[property_name.end..body.end]);
    let relative_group = group - body.start;
    put32(
        &mut renamed_body,
        relative_group,
        shifted(long(before, group) as usize, delta) as u32,
    );
    put32(
        &mut renamed_body,
        relative_group + 6,
        (new.len() + 6) as u32,
    );
    put16(&mut renamed_body, relative_group + 10, new.len() as u16);

    let mut after = before.to_vec();
    after[name.start..name.start + new.len()].copy_from_slice(&new);
    after[name.start + new.len()..shifted(end, delta)].copy_from_slice(&before[name.end..end]);
    put16(&mut after, name.start - 2, new.len() as u16);
    put32(
        &mut after,
        definition_base + DEFINITION_LENGTH,
        shifted(length, delta) as u32,
    );
    put16(
        &mut after,
        definition_base + FREE,
        shifted(word(before, definition_base + FREE), -delta) as u16,
    );

    // Keep the original backing bytes in newly freed space, including shrink
    // tails. Relocate the separate OFF payload before placing the live user body.
    after[shifted(database.start, -delta)..shifted(database.end, -delta)]
        .copy_from_slice(&before[database.start..database.end]);
    after[shifted(body.start, -delta)..body.end].copy_from_slice(&renamed_body);
    put16(
        &mut after,
        property_base + DIRECTORY + slot * 2,
        (shifted(body.start, -delta) - property_base) as u16,
    );
    put16(
        &mut after,
        property_base + DIRECTORY + (rows - 1) * 2,
        (shifted(database.start, -delta) - property_base) as u16,
    );
    put16(
        &mut after,
        property_base + FREE,
        shifted(word(before, property_base + FREE), -delta) as u16,
    );
    put32(&mut after, descriptor, SINGLE | renamed_body.len() as u32);
    after
}

#[cfg(all(test, feature = "access-support"))]
mod tests;
