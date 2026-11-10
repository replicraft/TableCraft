//! Independent retained-history RENAME after populated DROP and ADD.
use super::{
    DEFINITION, FIELD_BYTES, FIELD_COUNT, FIELD_START, FREE, LENGTH_MASK, NAME_LIMIT, PAGE,
    PROPERTY, SINGLE, catalog_descriptor, group_name, long, row, shifted, utf16, word,
};
use crate::fixtures::access::{
    canonical_created_number::mixed::{
        after_off::deleted_field::populated::{
            Case,
            readdition::{self, Target},
        },
        storage,
    },
    put16, put32,
};
use tablecraft_core::values::Value;

pub fn source(case: Case, target: Target, filled: bool) -> Vec<u8> {
    let declared = readdition::after(&readdition::source(case), target);
    if !filled {
        return declared;
    }
    match target {
        Target::Number => readdition::fill_number(case, &declared, 0, 123),
        Target::ShortText => readdition::fill_text(case, &declared, 0, "first"),
    }
}

pub fn expected(case: Case, target: Target, filled: bool) -> Vec<Vec<Value>> {
    let mut values = readdition::expected(case);
    if filled {
        *values[0].last_mut().unwrap() = match target {
            Target::Number => Value::Int32(123),
            Target::ShortText => Value::Text("first".into()),
        };
    }
    values
}

/// The old live body becomes immutable history; append its renamed copy and
/// repoint only its catalog descriptor. No producer or native codec is called.
pub fn rename(before: &[u8], old_name: &str, new_name: &str) -> Vec<u8> {
    assert!(old_name.is_ascii() && new_name.is_ascii());
    assert!((1..=NAME_LIMIT).contains(&new_name.len()));
    assert_eq!(before.len(), 11 * PAGE);
    if old_name == new_name {
        return before.to_vec();
    }
    let old = utf16(old_name);
    let new = utf16(new_name);
    let delta = new.len() as isize - old.len() as isize;
    let base = DEFINITION * PAGE;
    let active = word(before, base + FIELD_COUNT);
    let mut position = base + FIELD_START + active * FIELD_BYTES;
    let mut selected = None;
    for _ in 0..active {
        let length = word(before, position);
        let name = position + 2..position + 2 + length;
        if before[name.start..name.end] == old {
            assert!(selected.replace(name.start..name.end).is_none());
        }
        position = name.end;
    }
    let name = selected.unwrap();
    let length = long(before, base + 8) as usize;
    let end = base + length;
    assert!(name.end <= end && shifted(length, delta) <= PAGE - 8);
    let descriptor = catalog_descriptor(before);
    let tagged = long(before, descriptor);
    assert_eq!(tagged & !LENGTH_MASK, SINGLE);
    let reference = long(before, descriptor + 4);
    assert_eq!(reference >> 8, PROPERTY as u32);
    let body = row(before, PROPERTY, reference as usize & 255);
    assert_eq!(body.len(), (tagged & LENGTH_MASK) as usize);
    let property_name = group_name(before, &body, &old);
    let group = property_name.start - 12;
    let mut renamed = before[body.start..property_name.start].to_vec();
    renamed.extend(&new);
    renamed.extend_from_slice(&before[property_name.end..body.end]);
    let relative = group - body.start;
    put32(
        &mut renamed,
        relative,
        shifted(long(before, group) as usize, delta) as u32,
    );
    put32(&mut renamed, relative + 6, (new.len() + 6) as u32);
    put16(&mut renamed, relative + 10, new.len() as u16);

    let mut after = before.to_vec();
    after[name.start..name.start + new.len()].copy_from_slice(&new);
    after[name.start + new.len()..shifted(end, delta)].copy_from_slice(&before[name.end..end]);
    put16(&mut after, name.start - 2, new.len() as u16);
    put32(&mut after, base + 8, shifted(length, delta) as u32);
    put16(
        &mut after,
        base + FREE,
        shifted(word(before, base + FREE), -delta) as u16,
    );
    storage::append_current(before, after, &renamed)
}

/// Sparse variable identities with the legacy user-then-OFF live placement.
/// The current dictionary is fully used: Score owns Required/DefaultValue and
/// NextText owns Required/AllowZeroLength. Historical rows are not repacked.
pub fn adjacent_live_roles() -> Vec<u8> {
    const CATALOG_PAGE: usize = 4;
    const DIRECTORY: usize = 14;
    const ROW_COUNT_OFFSET: usize = 12;
    const OFFSET_MASK: usize = 0x1fff;
    const DATABASE_ID: u32 = 0x0f00_0004;
    const CATALOG_FIELDS: usize = 17;
    const FOOTER_BYTES: usize = 5;
    const PROPERTY_START: usize = 18;
    const PROPERTY_END: usize = 20;
    const DESCRIPTOR_BYTES: usize = 12;

    let before = source(Case::TextPresent, Target::ShortText, false);
    let selected = catalog_descriptor(&before);
    let user_reference = long(&before, selected + 4);
    let user = row(&before, PROPERTY, user_reference as usize & 255);
    let body = &before[user];
    let after = storage::append_current(&before, before.to_vec(), body);

    let mut database_descriptor = None;
    for slot in 0..word(&after, CATALOG_PAGE * PAGE + ROW_COUNT_OFFSET) {
        let raw = word(&after, CATALOG_PAGE * PAGE + DIRECTORY + 2 * slot);
        if raw & !OFFSET_MASK != 0 {
            continue;
        }
        let record = row(&after, CATALOG_PAGE, slot);
        if word(&after, record.start) == CATALOG_FIELDS
            && long(&after, record.start + 2) == DATABASE_ID
        {
            let footer = record.end - FOOTER_BYTES;
            let start = word(&after, footer - PROPERTY_START);
            let end = word(&after, footer - PROPERTY_END);
            assert_eq!(end - start, DESCRIPTOR_BYTES);
            assert!(database_descriptor.replace(record.start + start).is_none());
        }
    }
    let descriptor = database_descriptor.unwrap();
    let tagged = long(&after, descriptor);
    assert_eq!(tagged & !LENGTH_MASK, SINGLE);
    let reference = long(&after, descriptor + 4);
    assert_eq!(reference >> 8, PROPERTY as u32);
    let original = row(&after, PROPERTY, reference as usize & 255);
    assert_eq!(original.len(), (tagged & LENGTH_MASK) as usize);
    let off = after[original].to_vec();
    let live = after;
    let mut result = live.to_vec();
    let appended = storage::append_body(&live, &mut result, &off);
    put32(&mut result, descriptor + 4, appended);
    assert_eq!(
        appended as usize & 255,
        word(&result, PROPERTY * PAGE + ROW_COUNT_OFFSET) - 1
    );
    result
}
