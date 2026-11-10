//! Independent full-count historical row reconstruction, separate from placement.
use super::raw_schema_add::Kind;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

pub(in crate::fixtures::access) fn reconstruct(
    old: &[u8],
    declaration: &[u8],
    has_variables: bool,
    target: Kind,
    value: Option<&[u8]>,
) -> Vec<u8> {
    let count = word(old, 0);
    let fields = word(declaration, 41);
    let active = word(declaration, 45);
    let field = 75 + (active - 1) * 25;
    let retained_variables = word(declaration, 43);
    let number = word(declaration, field + 5);
    assert!(count <= fields && number < fields);
    let payload = value.unwrap_or(&[]);
    let mask = old.len() - count.div_ceil(8);
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
        Kind::Number => {
            assert_eq!(declaration[field], 4);
            let start = 2 + word(declaration, field + 21);
            let expanded = fixed_end.max(start + 4);
            row = old[..fixed_end].to_vec();
            row.resize(expanded, 0);
            if value.is_some() {
                row[start..start + 4].copy_from_slice(payload);
            }
            row.extend_from_slice(&old[fixed_end..data_end]);
            new_offsets = offsets
                .iter()
                .map(|offset| offset + expanded - fixed_end)
                .collect::<Vec<_>>();
            variables = retained_variables;
            new_offsets.resize(variables + 1, end + expanded - fixed_end);
        }
        Kind::ShortText => {
            assert_eq!(declaration[field], 10);
            let variable = word(declaration, field + 7);
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
    row
}
