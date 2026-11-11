//! Literal unadorned Currency descriptor insertion; no property snapshot is authored.
use crate::fixtures::access::{put16, put32};

const PAGE: usize = 4096;

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

/// Append one ordinary Currency declaration while preserving all catalog/property bytes.
pub fn after(before: &[u8], definition: u32, name: &str) -> Vec<u8> {
    assert!(name.is_ascii() && !name.is_empty() && name.len() <= 64);
    let base = definition as usize * PAGE;
    let old = &before[base..base + PAGE];
    assert_eq!(&old[47..55], &[0; 8]);
    let count = word(old, 45);
    let high = word(old, 41);
    let names = 63 + count * 25;
    let mut tail = names;
    for _ in 0..count {
        tail += 2 + word(old, tail);
    }
    let length = u32::from_le_bytes(old[8..12].try_into().unwrap()) as usize;
    let mut fixed = 0;
    let mut variable = 0;
    let mut creation = 0;
    for start in (63..names).step_by(25) {
        creation = creation.max(word(old, start + 9) + 1);
        if old[start + 15] & 1 != 0 {
            if old[start] != 1 {
                fixed = fixed.max(word(old, start + 21) + word(old, start + 23));
            }
        } else {
            variable = variable.max(word(old, start + 7) + 1);
        }
    }
    let mut descriptor = [0; 25];
    descriptor[0] = 5;
    put32(&mut descriptor, 1, 1625);
    put16(&mut descriptor, 5, high as u16);
    put16(&mut descriptor, 7, variable as u16);
    put16(&mut descriptor, 9, creation as u16);
    descriptor[15] = 3;
    put16(&mut descriptor, 21, fixed as u16);
    put16(&mut descriptor, 23, 8);
    let mut page = old[..names].to_vec();
    page.extend(descriptor);
    page.extend_from_slice(&old[names..tail]);
    page.extend(((name.len() * 2) as u16).to_le_bytes());
    page.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
    page.extend_from_slice(&old[tail..length]);
    let new_length = page.len();
    assert!(new_length <= PAGE - 8);
    put16(&mut page, 2, (PAGE - 8 - new_length) as u16);
    put32(&mut page, 8, new_length as u32);
    put16(&mut page, 41, (high + 1) as u16);
    put16(&mut page, 45, (count + 1) as u16);
    page.extend_from_slice(&old[new_length..]);
    let mut after = before.to_vec();
    after[base..base + PAGE].copy_from_slice(&page);
    after
}
