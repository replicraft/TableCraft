//! Independent TDEF field append from actual retained native counters and live offsets.
use super::{put16, put32};

const PAGE: usize = 4096;
const FIELD_START: usize = 75;
const FIELD_BYTES: usize = 25;

#[derive(Clone, Copy)]
pub(in crate::fixtures::access) enum Kind {
    Number,
    ShortText,
}

#[derive(Clone, Copy)]
pub(in crate::fixtures::access) enum Slack {
    Zero,
    Preserve,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

pub(in crate::fixtures::access) fn append(
    before: &[u8],
    definition: u32,
    name: &str,
    target: Kind,
    slack: Slack,
) -> Vec<u8> {
    let definition_start = definition as usize * PAGE;
    let old = &before[definition_start..definition_start + PAGE];
    let high = word(old, 41);
    let variable_high = word(old, 43);
    let active = word(old, 45);
    assert!(active <= high);
    let names = FIELD_START + FIELD_BYTES * active;
    let mut tail = names;
    for _ in 0..active {
        tail += 2 + word(old, tail);
    }
    let length = u32::from_le_bytes(old[8..12].try_into().unwrap()) as usize;
    let mut fixed = 0;
    let mut variable = 0;
    let mut next_id = 0;
    for at in (FIELD_START..names).step_by(FIELD_BYTES) {
        next_id = next_id.max(word(old, at + 9) + 1);
        if old[at + 15] & 1 != 0 {
            assert_eq!(old[at], 4);
            fixed = fixed.max(word(old, at + 21) + word(old, at + 23));
        } else {
            variable = variable.max(word(old, at + 7) + 1);
        }
    }

    let mut field = [0; FIELD_BYTES];
    put32(&mut field, 1, 1625);
    put16(&mut field, 5, high as u16);
    put16(&mut field, 7, variable as u16);
    put16(&mut field, 9, next_id as u16);
    match target {
        Kind::Number => {
            field[0] = 4;
            field[15] = 3;
            put16(&mut field, 21, fixed as u16);
            put16(&mut field, 23, 4);
        }
        Kind::ShortText => {
            field[0] = 10;
            field[11..15].copy_from_slice(&[9, 4, 0, 1]);
            field[15] = 2;
            field[16] = 1;
            put16(&mut field, 23, 510);
        }
    }
    let mut definition = old[..names].to_vec();
    definition.extend(field);
    definition.extend_from_slice(&old[names..tail]);
    let label: Vec<_> = name.encode_utf16().collect();
    definition.extend((label.len() as u16 * 2).to_le_bytes());
    for unit in label {
        definition.extend(unit.to_le_bytes());
    }
    definition.extend_from_slice(&old[tail..length]);
    let size = definition.len();
    put16(&mut definition, 2, (PAGE - size - 8) as u16);
    put32(&mut definition, 8, size as u32);
    put16(&mut definition, 41, (high + 1) as u16);
    put16(&mut definition, 45, (active + 1) as u16);
    if matches!(target, Kind::ShortText) {
        put16(&mut definition, 43, (variable_high + 1) as u16);
    }
    definition.resize(PAGE, 0);
    if matches!(slack, Slack::Preserve) {
        definition[size..].copy_from_slice(&old[size..]);
    }
    let mut output = before.to_vec();
    output[definition_start..definition_start + PAGE].copy_from_slice(&definition);
    output
}
