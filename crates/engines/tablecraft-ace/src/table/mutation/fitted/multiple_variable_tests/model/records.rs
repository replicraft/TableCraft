//! Literal C5/V3 rows and one jointly computed final variable footer.
use super::{Family, Row, Serialization};

const PADDING: [u8; 8] = [0x9d, 0xe1, 0xa7, 0, 0x8f, 0xb2, 0x43, 0xc9];

fn payload(value: &[u8], text: bool, compact: bool) -> Vec<u8> {
    if !text {
        return value.to_vec();
    }
    if compact {
        return [vec![0xff, 0xfe], value.to_vec()].concat();
    }
    value
        .iter()
        .flat_map(|byte| u16::from(*byte).to_le_bytes())
        .collect()
}

pub(super) fn original(row: &Row, family: Family, raw: bool, slack: bool) -> Vec<u8> {
    let mut bytes = 5_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(row.age.unwrap_or(0x6655_4433).to_le_bytes());
    let first = row.first.as_ref().map_or_else(
        || {
            if raw {
                vec![0xd3, 0x91]
            } else {
                vec![0xd3, 0, 0xff, 0x91]
            }
        },
        |value| payload(value, family.first_text(), family.first_text() && !raw),
    );
    let second = row.second.as_ref().map_or_else(
        || vec![0x87, 0xa6, 0x33],
        |value| payload(value, family.second_text(), family.second_text() && !raw),
    );
    bytes.extend(first);
    let first_end = bytes.len();
    bytes.extend(second);
    let second_end = bytes.len();
    bytes.extend(
        row.opaque
            .as_ref()
            .map_or(&[0xa5, 0xde, 0x77][..], Vec::as_slice),
    );
    let end = bytes.len();
    if slack {
        bytes.extend(PADDING);
    }
    for offset in [end, second_end, first_end, 10] {
        bytes.extend((offset as u16).to_le_bytes());
    }
    bytes.extend(3_u16.to_le_bytes());
    bytes.push(mask(row));
    bytes
}

fn mask(row: &Row) -> u8 {
    0xa1 | if row.age.is_some() { 2 } else { 0 }
        | if row.first.is_some() { 4 } else { 0 }
        | if row.second.is_some() { 8 } else { 0 }
        | if row.opaque.is_some() { 16 } else { 0 }
}

fn spans(bytes: &[u8]) -> [usize; 4] {
    let footer = bytes.len() - 11;
    std::array::from_fn(|position| {
        let at = footer + (3 - position) * 2;
        usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
    })
}

pub(super) fn final_record(
    before: &[u8],
    old: &Row,
    new: &Row,
    family: Family,
    policy: Serialization,
) -> Vec<u8> {
    let offsets = spans(before);
    let mut selected = Vec::new();
    let mut parts = Vec::new();
    for (position, (old_value, new_value, text)) in [
        (&old.first, &new.first, family.first_text()),
        (&old.second, &new.second, family.second_text()),
    ]
    .into_iter()
    .enumerate()
    {
        if old_value == new_value {
            parts.push(before[offsets[position]..offsets[position + 1]].to_vec());
        } else {
            let compact = text && matches!(policy, Serialization::Fitted)
                || text && new_value.as_ref().is_some_and(|value| value.len() > 2);
            parts.push(
                new_value
                    .as_ref()
                    .map_or_else(Vec::new, |value| payload(value, text, compact)),
            );
            selected.push(position);
        }
    }
    parts.push(before[offsets[2]..offsets[3]].to_vec());
    let capacity = before.len() - 11 - 10;
    if matches!(policy, Serialization::Fitted) {
        let mut length = parts.iter().map(Vec::len).sum::<usize>();
        for position in selected {
            if length <= capacity {
                break;
            }
            let value = if position == 0 {
                &new.first
            } else {
                &new.second
            };
            let text = if position == 0 {
                family.first_text()
            } else {
                family.second_text()
            };
            if text && let Some(value) = value {
                let raw = payload(value, true, false);
                if raw.len() < parts[position].len() {
                    length -= parts[position].len() - raw.len();
                    parts[position] = raw;
                }
            }
        }
        assert!(length <= capacity, "literal fitted image must fit jointly");
    }
    let logical = 10 + parts.iter().map(Vec::len).sum::<usize>();
    let length = before.len().max(logical + 11);
    let mut after = before.to_vec();
    after.resize(length, 0);
    let mut ends = [10; 4];
    let mut end = 10;
    for (position, bytes) in parts.iter().enumerate() {
        after[end..end + bytes.len()].copy_from_slice(bytes);
        end += bytes.len();
        ends[position + 1] = end;
    }
    let footer = length - 11;
    for (position, end) in ends.into_iter().rev().enumerate() {
        after[footer + position * 2..footer + position * 2 + 2]
            .copy_from_slice(&(end as u16).to_le_bytes());
    }
    after[footer + 8..footer + 10].copy_from_slice(&3_u16.to_le_bytes());
    after[length - 1] = mask(new);
    if let Some(age) = new.age {
        after[6..10].copy_from_slice(&age.to_le_bytes());
    }
    after
}
