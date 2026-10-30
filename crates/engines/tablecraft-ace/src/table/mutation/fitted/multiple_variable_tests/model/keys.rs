//! Reuse the actual independent variable-key facts, not native key encoders.
use super::{Family, Row, frames};

fn integer(value: Option<i32>, descending: bool) -> Vec<u8> {
    let Some(value) = value else {
        return vec![if descending { 255 } else { 0 }];
    };
    let mut bytes = vec![if descending { 128 } else { 127 }];
    bytes.extend(
        ((value as u32) ^ 0x8000_0000)
            .to_be_bytes()
            .map(|byte| if descending { !byte } else { byte }),
    );
    bytes
}

pub(super) fn entries(
    rows: &[Row],
    family: Family,
    descending: bool,
    physical: usize,
) -> Vec<Vec<u8>> {
    let mut entries = rows
        .iter()
        .enumerate()
        .map(|(ordinal, row)| {
            let first =
                frames::keys::name_key(row.first.as_deref(), family.first_key(), descending);
            let second =
                frames::keys::name_key(row.second.as_deref(), family.second_key(), !descending);
            let mut bytes = if physical == 0 {
                [first, second].concat()
            } else {
                [second, first, integer(row.age, !descending)].concat()
            };
            bytes.extend(((12_u32 << 8) | ordinal as u32).to_be_bytes());
            bytes
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        a[..a.len() - 4]
            .cmp(&b[..b.len() - 4])
            .then(a[a.len() - 4..].cmp(&b[b.len() - 4..]))
    });
    entries
}
