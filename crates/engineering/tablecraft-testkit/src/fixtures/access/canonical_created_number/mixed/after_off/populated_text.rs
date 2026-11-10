//! Independent metadata-only Short Text ADD over unchanged historical records.
pub mod insertion;
pub mod present;

use super::{SourceCase, source};
use crate::fixtures::access::{canonical_initial_table_creation::inserts::long_rows, put32};

/// Existing nullable Number records precede the new variable declaration.
/// Physical DATA/INDEX bytes and original generation counters remain exact.
pub fn pair(name: &str, values: &[Option<i32>]) -> [Vec<u8>; 2] {
    let before = long_rows(&source(SourceCase::Number), values);
    let after = append_text(&before, name);
    [before, after]
}

/// The independent raw declaration/property append also applies to populated
/// sources: unlike creation from values, it never consults or rebuilds records.
pub fn append_text(before: &[u8], name: &str) -> Vec<u8> {
    append_text_declaration(before, name, 255, true)
}

/// Literal alternate capacities/compression, with exactly the same NULL/default policy.
pub fn append_text_declaration(before: &[u8], name: &str, units: u16, compressed: bool) -> Vec<u8> {
    let mut metadata = before.to_vec();
    for offset in [16, 20, 67] {
        put32(&mut metadata, 7 * 4096 + offset, 0);
    }
    let mut after = super::super::append_text_definition(&metadata, name, units, compressed);
    for offset in [16, 20, 67] {
        let at = 7 * 4096 + offset;
        after[at..at + 4].copy_from_slice(&before[at..at + 4]);
    }
    let original_body = super::super::storage::selected_body(before);
    let body = super::super::properties::append_text(&original_body, name);
    super::super::storage::append(before, after, &body)
}

/// First-party mixed records retain raw compressed/wide/NULL payload and padding.
pub fn mixed_pair() -> [Vec<u8>; 2] {
    let before = super::populated_mixed::pair(super::populated_mixed::Case::Text)[0].to_vec();
    let after = append_text(&before, "NewLabel");
    [before, after]
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// Independently append the final declared Text slot to one direct-root record.
/// Old field payload bytes, including ignored NULL bytes and padding, stay exact.
pub fn after(before: &[u8], slot: usize, value: &str) -> Vec<u8> {
    const PAGE: usize = 4096;
    const DATA: usize = 8 * PAGE;
    const DEFINITION: usize = 7 * PAGE;
    let fields = word(before, DEFINITION + 41);
    let start = word(before, DATA + 14 + slot * 2);
    assert!([0, 0x4000].contains(&(start & !0x1fff)));
    let end = if slot == 0 {
        PAGE
    } else {
        word(before, DATA + 14 + (slot - 1) * 2) & 0x1fff
    };
    let forwarded = start & 0x4000 != 0;
    let old = if forwarded {
        assert_eq!(slot, 0);
        super::populated::materialization::forwarded::record(before)
    } else {
        &before[DATA + start..DATA + end]
    };
    let count = word(old, 0);
    assert!(count < fields);
    let last = DEFINITION + 75 + (fields - 1) * 25;
    assert_eq!(before[last], 10);
    let variables = word(before, DEFINITION + 43);
    assert_eq!(word(before, last + 7), variables - 1);
    let mut fixed = 2;
    let mut expanded = 2;
    let mut old_variables = 0;
    for number in 0..fields {
        let field = DEFINITION + 75 + number * 25;
        if before[field + 15] & 1 != 0 {
            let stop = 2 + word(before, field + 21) + word(before, field + 23);
            expanded = expanded.max(stop);
            if number < count {
                fixed = fixed.max(stop);
            }
        } else if number < count {
            old_variables += 1;
        }
    }
    let old_mask = old.len() - count.div_ceil(8);
    let data_end = if old_variables == 0 {
        old_mask
    } else {
        old_mask - 2 - 2 * (old_variables + 1)
    };
    let offsets = if old_variables == 0 {
        Vec::new()
    } else {
        assert_eq!(word(old, old_mask - 2), old_variables);
        (0..=old_variables)
            .map(|index| word(old, old_mask - 4 - index * 2))
            .collect::<Vec<_>>()
    };
    let payload_end = offsets.last().copied().unwrap_or(fixed);
    let mut payload = Vec::new();
    let units: Vec<_> = value.encode_utf16().collect();
    assert!(units.len() <= word(before, last + 23) / 2);
    if before[last + 16] & 1 != 0
        && units.len() > 2
        && units.iter().all(|unit| (1..=255).contains(unit))
    {
        payload.extend([255, 254]);
        payload.extend(units.iter().map(|unit| *unit as u8));
    } else {
        assert!(!value.starts_with('\u{feff}'));
        for unit in units {
            payload.extend(unit.to_le_bytes());
        }
    }
    let mut row = (fields as u16).to_le_bytes().to_vec();
    row.extend_from_slice(&old[2..fixed]);
    row.resize(expanded, 0);
    row.extend_from_slice(&old[fixed..payload_end]);
    row.extend_from_slice(&payload);
    row.extend_from_slice(&old[payload_end..data_end]);
    let delta = expanded - fixed;
    for variable in (0..=variables).rev() {
        let offset = if variable <= old_variables {
            offsets.get(variable).copied().unwrap_or(fixed) + delta
        } else {
            payload_end
                + delta
                + if variable == variables {
                    payload.len()
                } else {
                    0
                }
        };
        row.extend((offset as u16).to_le_bytes());
    }
    row.extend((variables as u16).to_le_bytes());
    for byte in 0..fields.div_ceil(8) {
        let mut bits = old.get(old_mask + byte).copied().unwrap_or(0);
        for bit in 0..8 {
            let number = byte * 8 + bit;
            if number >= count && number < fields {
                bits &= !(1 << bit);
                if number == fields - 1 {
                    bits |= 1 << bit;
                }
            }
        }
        row.push(bits);
    }
    if forwarded {
        super::populated::materialization::forwarded::place(before, &row)
    } else {
        super::populated::materialization::place_record(before, slot, &row)
    }
}

/// An already forwarded Number record precedes the new Text declaration.
pub fn forwarded_pair() -> [Vec<u8>; 2] {
    let before = super::populated::materialization::forwarded::pair()[0].to_vec();
    let after = append_text(&before, "NewLabel");
    [before, after]
}

/// A generated-ID-only table has no incidental unindexed LONG companion.
pub fn id_only_pair() -> [Vec<u8>; 2] {
    use crate::fixtures::access::{
        canonical_created_number, canonical_database_options,
        canonical_initial_table_creation::inserts::initial_rows_image,
    };
    let empty = canonical_database_options::explicit_off(&canonical_created_number::source(
        canonical_created_number::Source::IdOnly,
    ));
    let records = (1_i32..=3)
        .map(|id| {
            let mut record = vec![1, 0];
            record.extend(id.to_le_bytes());
            record.push(1);
            (0, record)
        })
        .collect();
    let before = initial_rows_image(&empty, records);
    let after = append_text(&before, "NewLabel");
    [before, after]
}
