//! Exact appended native17 catalog framing, independent of the record writer.
use super::super::AddTableError;
use crate::{InspectError, TableProperties, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const PAGE_BYTES: usize = 4096;
const FIXED_BYTES: usize = 32;
const FOOTER_BYTES: usize = 29;
const VARIABLE_FIELDS: usize = 11;
const PROPERTY_VARIABLE: usize = 8;
const INLINE_HEADER_BYTES: usize = 12;
const INLINE: u32 = 0x8000_0000;
const TABLES_ID: u32 = 0x0f00_0001;

pub(in crate::creation::add_table) fn verify(
    raw: &[u8],
    name: &str,
    native_id: u32,
    properties: &TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    verify_at(
        raw, name, native_id, properties, TABLES_ID, budget, cancelled,
    )
}

pub(in crate::creation::add_table) fn verify_at(
    raw: &[u8],
    name: &str,
    native_id: u32,
    properties: &TableProperties,
    tables_parent: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    check(budget, cancelled)?;
    budget
        // Reserve the bounded initialization and reconstructed row writes;
        // the final actual-length comparison is charged separately below.
        .charge(Resource::Steps, (2 * PAGE_BYTES) as u64)
        .map_err(InspectError::from)?;
    let mut expected = [0; PAGE_BYTES];
    expected[..2].copy_from_slice(&17_u16.to_le_bytes());
    expected[2..6].copy_from_slice(&native_id.to_le_bytes());
    expected[6..10].copy_from_slice(&tables_parent.to_le_bytes());
    expected[10..12].copy_from_slice(&1_u16.to_le_bytes());
    let mut at = FIXED_BYTES;
    for unit in name.encode_utf16() {
        check(budget, cancelled)?;
        expected[at..at + 2].copy_from_slice(&unit.to_le_bytes());
        at += 2;
    }
    let name_end = at;
    let properties = properties.raw_bytes();
    if !properties.is_empty() {
        if at + INLINE_HEADER_BYTES + properties.len() + FOOTER_BYTES > PAGE_BYTES {
            return Err(AddTableError::Preservation);
        }
        expected[at..at + 4].copy_from_slice(&(INLINE | properties.len() as u32).to_le_bytes());
        at += INLINE_HEADER_BYTES;
        expected[at..at + properties.len()].copy_from_slice(properties);
        at += properties.len();
    }
    let end = at;
    expected[at..at + 2].copy_from_slice(&(end as u16).to_le_bytes());
    at += 2;
    for slot in (0..VARIABLE_FIELDS).rev() {
        let start = if slot == 0 {
            FIXED_BYTES
        } else if !properties.is_empty() && slot <= PROPERTY_VARIABLE {
            name_end
        } else {
            end
        };
        expected[at..at + 2].copy_from_slice(&(start as u16).to_le_bytes());
        at += 2;
    }
    expected[at..at + 2].copy_from_slice(&(VARIABLE_FIELDS as u16).to_le_bytes());
    at += 2;
    expected[at..at + 3].copy_from_slice(&[0xbf, if properties.is_empty() { 0 } else { 0x40 }, 0]);
    at += 3;
    budget
        .charge(Resource::Steps, at as u64)
        .map_err(InspectError::from)?;
    if raw != &expected[..at] {
        return Err(AddTableError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
