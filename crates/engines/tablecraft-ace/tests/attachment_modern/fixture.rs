//! Independent modern complex child layout; no donated names or payload bytes.
use crate::{old_attachment, support};

pub const PAGE: usize = 4096;
pub const FLAT: usize = 20;
pub const DATA: usize = 22;
pub const FIRST_FIELD: usize = 63;
pub const FIELD_BYTES: usize = 25;
pub const ID_FIELD: usize = FIRST_FIELD + 7 * FIELD_BYTES;

pub fn inline(payload: &[u8]) -> Vec<u8> {
    let mut bytes = (0x80000000 | payload.len() as u32).to_le_bytes().to_vec();
    bytes.extend([0; 8]);
    bytes.extend(payload);
    bytes
}

fn text(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

pub fn row(owner: i32, identity: i32, variant: usize, file_data: &[u8]) -> Vec<u8> {
    let mut body = 8_u16.to_le_bytes().to_vec();
    body.extend(i32::MIN.to_le_bytes());
    body.extend((-0.25_f64).to_bits().to_le_bytes());
    assert_eq!(body.len(), 14);
    let values = match variant {
        0 => [
            owner.to_le_bytes().to_vec(),
            file_data.to_vec(),
            text("synthetic.bin"),
            text("field/type"),
            inline(&text("relative?x")),
            identity.to_le_bytes().to_vec(),
        ],
        1 => [
            owner.to_le_bytes().to_vec(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            identity.to_le_bytes().to_vec(),
        ],
        2 => [
            owner.to_le_bytes().to_vec(),
            inline(&[]),
            Vec::new(),
            Vec::new(),
            inline(&[]),
            identity.to_le_bytes().to_vec(),
        ],
        _ => unreachable!(),
    };
    let mut offsets = vec![14_u16];
    for value in values {
        body.extend(value);
        offsets.push(body.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        body.extend(offset.to_le_bytes());
    }
    body.extend(6_u16.to_le_bytes());
    // Row1 deliberately retains nonzero fixed payload behind absent bits.
    body.push(if variant == 1 { 0x81 } else { 0xff });
    body
}

pub fn database(code: u8) -> Vec<u8> {
    with_first_data(code, &inline(old_attachment::PAYLOAD))
}

pub fn with_first_data(code: u8, file_data: &[u8]) -> Vec<u8> {
    let mut bytes = old_attachment::database();
    bytes[20] = code;
    let fields = [
        ("Owner", 4, 0, 4),
        ("FileData", 11, 0, 0),
        ("FileFlags", 4, 0, 4),
        ("FileName", 10, 0, 510),
        ("FileTimeStamp", 8, 4, 8),
        ("FileType", 10, 0, 510),
        ("FileURL", 12, 0, 0),
        ("ChildID", 4, 0, 4),
    ];
    let mut flat = old_attachment::definition(&fields, 3, 21 << 8, false);
    for position in 0..fields.len() {
        let at = FIRST_FIELD + position * FIELD_BYTES;
        flat[at + 16] = 0x10;
    }
    for (position, variable) in [(0, 0_u16), (1, 1), (3, 2), (5, 3), (6, 4), (7, 5)] {
        let at = FIRST_FIELD + position * FIELD_BYTES;
        flat[at + 15] = if position == 7 { 6 } else { 2 };
        flat[at + 7..at + 9].copy_from_slice(&variable.to_le_bytes());
    }
    flat[FIRST_FIELD + 16] = 8;
    flat[ID_FIELD + 16] = 4;
    flat[43..45].copy_from_slice(&6_u16.to_le_bytes());
    bytes[FLAT * PAGE..(FLAT + 1) * PAGE].copy_from_slice(&flat);
    install_rows(
        &mut bytes,
        &[
            row(10, 1, 0, file_data),
            row(10, 2, 1, &[]),
            row(20, 3, 2, &[]),
        ],
    );
    bytes
}

pub fn install_rows(bytes: &mut [u8], rows: &[Vec<u8>]) {
    let page = support::data_page(
        FLAT as u32,
        &rows.iter().cloned().map(|row| (0, row)).collect::<Vec<_>>(),
    );
    bytes[DATA * PAGE..(DATA + 1) * PAGE].copy_from_slice(&page);
}

pub fn span(bytes: &[u8], slot: usize) -> (usize, usize) {
    let base = DATA * PAGE;
    let start = u16::from_le_bytes(
        bytes[base + 14 + 2 * slot..base + 16 + 2 * slot]
            .try_into()
            .unwrap(),
    );
    let end = if slot == 0 {
        PAGE
    } else {
        u16::from_le_bytes(
            bytes[base + 12 + 2 * slot..base + 14 + 2 * slot]
                .try_into()
                .unwrap(),
        ) as usize
    };
    (base + usize::from(start), base + end)
}
