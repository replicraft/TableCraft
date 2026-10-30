//! Literal sixteen-field rows; no mutation encoder supplies expected bytes.
use super::{PAGE, raw};

const EXTENDED_EPOCH: &[u8; 42] = b"0000000000000000000:0000000000000000000:7\0";
const EXTENDED_TICK: &[u8; 42] = b"0000000000000000000:0000000000000000001:7\0";

pub(super) fn inline(payload: &[u8]) -> Vec<u8> {
    let mut bytes = (0x8000_0000 | payload.len() as u32).to_le_bytes().to_vec();
    bytes.extend([0; 8]);
    bytes.extend(payload);
    bytes
}

fn external(length: u32, page: u32) -> Vec<u8> {
    let mut bytes = (0x4000_0000 | length).to_le_bytes().to_vec();
    bytes.extend((page << 8).to_le_bytes());
    bytes.extend([0; 4]);
    bytes
}

pub(super) fn text(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn calculated(value: i32) -> Vec<u8> {
    let mut bytes = vec![0; 16];
    bytes.extend(4_u32.to_le_bytes());
    bytes.extend(value.to_le_bytes());
    bytes.extend([0; 3]);
    bytes
}

pub(super) fn record(ordinal: usize) -> Vec<u8> {
    let id = ordinal as i32 + 1;
    let mut bytes = 16_u16.to_le_bytes().to_vec();
    bytes.extend(id.to_le_bytes());
    bytes.extend(id.to_le_bytes());
    bytes.extend((i64::from(id) * 1000).to_le_bytes());
    bytes.extend(0_f64.to_bits().to_le_bytes());
    bytes.extend(0.5_f64.to_bits().to_le_bytes());
    bytes.extend(if ordinal == 1 {
        &[0xa5; 42]
    } else {
        EXTENDED_EPOCH
    });
    bytes.extend(EXTENDED_TICK);
    bytes.extend((i64::from(id) * 12_345).to_le_bytes());
    bytes.extend((id * 10).to_le_bytes());
    assert_eq!(bytes.len(), 130);

    let short = match ordinal {
        0 => text("alpha"),
        1 => vec![0xd1, 0xe2],
        _ => text("omega"),
    };
    let memo = match ordinal {
        0 => external(8, 24),
        1 => vec![0xd3; 12],
        _ => inline(&text("tail")),
    };
    let ole = match ordinal {
        0 => external(3, 25),
        1 => vec![0xd4; 12],
        _ => inline(&[]),
    };
    let hyperlink = match ordinal {
        0 => external(text("label#https://invalid.test/#").len() as u32, 26),
        1 => vec![0xd5; 12],
        _ => inline(&text("synthetic#relative#")),
    };
    let mut offsets = vec![130_u16];
    for payload in [short, memo, ole, hyperlink, calculated(id * 2)] {
        bytes.extend(payload);
        offsets.push(bytes.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        bytes.extend(offset.to_le_bytes());
    }
    bytes.extend(5_u16.to_le_bytes());

    let mut mask = 0xffff_u16;
    // Booleans use mask bits as values; neither has a fixed payload span.
    mask &= !(1 << 10);
    if ordinal != 1 {
        mask &= !(1 << 11);
    }
    if ordinal == 1 {
        for number in [1, 2, 7, 12, 13] {
            mask &= !(1 << number);
        }
    }
    bytes.extend(mask.to_le_bytes());
    bytes
}

pub(super) fn page(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut page = raw::data_page(owner, rows);
    let payload: usize = rows.iter().map(|(_, body)| body.len()).sum();
    raw::put16(&mut page, 2, (PAGE - 14 - rows.len() * 2 - payload) as u16);
    page
}

pub(super) fn lval(payload: &[u8]) -> Vec<u8> {
    let mut page = page(0, &[(0, payload.to_vec()), (0, vec![0xf1, 0xe2, 0xd3])]);
    page[4..8].copy_from_slice(b"LVAL");
    page
}

pub(super) fn map(members: &[usize]) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for &number in members {
        bytes[5 + number / 8] |= 1 << (number % 8);
    }
    bytes
}
