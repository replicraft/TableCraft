//! Literal metadata ADD after a populated tail DROP, without rebuilding records.
use super::super::super::super::{properties, storage};
use super::{Case, expected_rows, pair};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const FIELD_START: usize = 75;
const FIELD_BYTES: usize = 25;

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

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

pub fn source(case: Case) -> Vec<u8> {
    pair(case)[1].to_vec()
}

/// Reuse the reference's surviving physical offsets, but give the declaration
/// the retained next logical number. Existing records therefore stay NULL.
pub fn after(before: &[u8], target: Target) -> Vec<u8> {
    assert_eq!(before.len(), 11 * PAGE);
    assert!(word(before, DEFINITION + 45) < word(before, DEFINITION + 41));
    let kind = match target {
        Target::Number => crate::fixtures::access::raw_schema_add::Kind::Number,
        Target::ShortText => crate::fixtures::access::raw_schema_add::Kind::ShortText,
    };
    let output = crate::fixtures::access::raw_schema_add::append(before, 7, target.name(), kind);
    let original = storage::current_body(before);
    let body = match target {
        Target::Number => properties::append_number(&original, target.name()),
        Target::ShortText => properties::append_text(&original, target.name()),
    };
    storage::append_current(before, output, &body)
}

pub fn expected(case: Case) -> Vec<Vec<Value>> {
    let mut rows = expected_rows(case);
    for row in &mut rows {
        row.push(Value::Null);
    }
    rows
}

/// Store an explicit Number in a newly added logical field; retired physical
/// bytes are not interpreted as its old value and sibling values remain exact.
pub fn fill_number(case: Case, before: &[u8], root: usize, value: i32) -> Vec<u8> {
    update(
        Stored::Historical(case),
        before,
        root,
        Target::Number,
        Some(&value.to_le_bytes()),
    )
}

/// Store explicit empty/compressed/wide Text through the selected reused or new
/// physical variable slot, retaining every unrelated raw slot and NULL payload.
pub fn fill_text(case: Case, before: &[u8], root: usize, value: &str) -> Vec<u8> {
    update(
        Stored::Historical(case),
        before,
        root,
        Target::ShortText,
        Some(&text_bytes(value)),
    )
}

/// Replace a physically present readded Text, retaining full-count geometry and
/// distinguishing an explicit empty value from a cleared NULL carrier.
pub fn replace_text(before: &[u8], root: usize, value: Option<&str>) -> Vec<u8> {
    let bytes = value.map(text_bytes);
    update(
        Stored::Present,
        before,
        root,
        Target::ShortText,
        bytes.as_deref(),
    )
}

fn text_bytes(value: &str) -> Vec<u8> {
    let units: Vec<_> = value.encode_utf16().collect();
    assert!(units.len() <= 255);
    let mut bytes = Vec::new();
    if units.len() > 2 && units.iter().all(|unit| (1..=255).contains(unit)) {
        bytes.extend([255, 254]);
        bytes.extend(units.iter().map(|unit| *unit as u8));
    } else {
        assert!(!value.starts_with('\u{feff}'));
        for unit in units {
            bytes.extend(unit.to_le_bytes());
        }
    }
    bytes
}

#[derive(Clone, Copy)]
enum Stored {
    Historical(Case),
    Present,
}

fn update(
    stored: Stored,
    before: &[u8],
    root: usize,
    target: Target,
    value: Option<&[u8]>,
) -> Vec<u8> {
    use super::super::super::populated_text::present;

    let old = present::record(before, root);
    let count = word(old, 0);
    let active = word(before, DEFINITION + 45);
    let field = DEFINITION + FIELD_START + (active - 1) * FIELD_BYTES;
    // Full records carry the retained variable-count high-water, including
    // deleted slots. Missing slots are empty, without inventing deleted types.
    let retained_variables = word(before, DEFINITION + 43);
    let number = word(before, field + 5);
    let fields = word(before, DEFINITION + 41);
    match stored {
        Stored::Historical(_) => assert!(number >= count),
        Stored::Present => {
            assert_eq!(count, fields);
            assert!(number < count);
        }
    }
    let payload = value.unwrap_or(&[]);
    let mask = old.len() - count.div_ceil(8);

    // Historical declarations are independently known in this raw source family,
    // including the now-retired field; no current production decoder is involved.
    let has_variables = match stored {
        Stored::Historical(case) => {
            let original = super::source(case);
            let old_active = word(&original, DEFINITION + 45);
            (0..old_active).any(|index| {
                let at = DEFINITION + FIELD_START + index * FIELD_BYTES;
                original[at + 15] & 1 == 0 && word(&original, at + 5) < count
            })
        }
        Stored::Present => retained_variables != 0,
    };
    let old_variables = if has_variables {
        word(old, mask - 2)
    } else {
        0
    };
    let data_end = if has_variables {
        mask - 2 - 2 * (old_variables + 1)
    } else {
        mask
    };
    let offsets: Vec<_> = if has_variables {
        (0..=old_variables)
            .map(|index| word(old, mask - 4 - 2 * index))
            .collect()
    } else {
        vec![data_end]
    };
    let fixed_end = offsets[0];
    let end = offsets[old_variables];
    let mut row;
    let mut new_offsets;
    let variables;
    match target {
        Target::Number => {
            assert_eq!(before[field], 4);
            let start = 2 + word(before, field + 21);
            let expanded = fixed_end.max(start + 4);
            row = old[..fixed_end].to_vec();
            row.resize(expanded, 0);
            row[start..start + 4].copy_from_slice(payload);
            row.extend_from_slice(&old[fixed_end..data_end]);
            new_offsets = offsets
                .iter()
                .map(|offset| offset + expanded - fixed_end)
                .collect::<Vec<_>>();
            variables = retained_variables;
            new_offsets.resize(variables + 1, end + expanded - fixed_end);
        }
        Target::ShortText => {
            assert_eq!(before[field], 10);
            let variable = word(before, field + 7);
            variables = retained_variables;
            let start = offsets.get(variable).copied().unwrap_or(end);
            let stop = offsets.get(variable + 1).copied().unwrap_or(end);
            row = old[..start].to_vec();
            row.extend_from_slice(payload);
            row.extend_from_slice(&old[stop..data_end]);
            new_offsets = Vec::new();
            for index in 0..=variables {
                let offset = if index <= variable {
                    offsets.get(index).copied().unwrap_or(end)
                } else {
                    offsets.get(index).copied().unwrap_or(end) - (stop - start) + payload.len()
                };
                new_offsets.push(offset);
            }
        }
    }
    row[..2].copy_from_slice(&(fields as u16).to_le_bytes());
    // Keep original physical footprint when replacement shrinks, so no sibling
    // slot moves and newly unused slack is explicit rather than resurrected data.
    row.resize(row.len().max(data_end), 0);
    if variables > 0 {
        for offset in new_offsets.into_iter().rev() {
            row.extend((offset as u16).to_le_bytes());
        }
        row.extend((variables as u16).to_le_bytes());
    }
    let next_mask = row.len();
    row.extend_from_slice(&old[mask..]);
    row.resize(next_mask + fields.div_ceil(8), 0);
    for logical in count..fields {
        row[next_mask + logical / 8] &= !(1 << (logical % 8));
    }
    row[next_mask + number / 8] &= !(1 << (number % 8));
    if value.is_some() {
        row[next_mask + number / 8] |= 1 << (number % 8);
    }
    crate::fixtures::access::raw_record_placement::place(before, 8, root, &row)
}
