use super::{calculated_fixture, support};

pub const PAGE_BYTES: usize = 4096;
pub const FIELD_START: usize = 10 * PAGE_BYTES + 63 + 25;

pub fn wrapped(text: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 16];
    bytes.extend((text.len() as u32).to_le_bytes());
    bytes.extend(text);
    bytes.extend([0; 3]);
    bytes
}

pub fn descriptor(tag: u32, length: usize, reference: u32) -> Vec<u8> {
    [
        (tag | length as u32).to_le_bytes(),
        reference.to_le_bytes(),
        0_u32.to_le_bytes(),
    ]
    .concat()
}

pub fn inline(bytes: &[u8]) -> Vec<u8> {
    let mut definition = descriptor(0x8000_0000, bytes.len(), 0);
    definition.extend(bytes);
    definition
}

pub fn lval(bytes: &[u8]) -> Vec<u8> {
    let mut page = support::data_page(0, &[(0, bytes.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    page
}

pub fn database(
    code: u8,
    compressed: bool,
    definitions: &[Option<Vec<u8>>],
    pages: &[(usize, Vec<u8>)],
) -> Vec<u8> {
    let mut bytes = calculated_fixture::database(&vec![None; definitions.len()]);
    bytes[20] = code;
    let property = bytes
        .windows(9)
        .position(|part| part == [9, 0, 0, 2, 1, 0, 1, 0, 4])
        .unwrap();
    bytes[property + 8] = 12;
    bytes[FIELD_START] = 12;
    bytes[FIELD_START + 16] = if compressed { 0xc1 } else { 0xc0 };
    support::put16(&mut bytes, FIELD_START + 23, 0);
    let rows = definitions
        .iter()
        .enumerate()
        .map(|(ordinal, definition)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend((ordinal as i32 + 1).to_le_bytes());
            if let Some(definition) = definition {
                row.extend(definition);
            }
            row.extend((row.len() as u16).to_le_bytes());
            row.extend(6_u16.to_le_bytes());
            row.extend(1_u16.to_le_bytes());
            row.push(if definition.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * PAGE_BYTES..13 * PAGE_BYTES].copy_from_slice(&support::data_page(10, &rows));
    if let Some(maximum) = pages.iter().map(|(number, _)| *number).max() {
        bytes.resize((maximum + 1) * PAGE_BYTES, 0);
    }
    for (number, page) in pages {
        bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES].copy_from_slice(page);
    }
    bytes
}

pub fn chained_database(code: u8, compressed: bool, text: &[u8]) -> Vec<u8> {
    let payload = wrapped(text);
    let chunks = payload.chunks(4000).collect::<Vec<_>>();
    let mut pages = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        let next = if index + 1 == chunks.len() {
            0
        } else {
            ((14 + index) as u32) << 8
        };
        let body = [next.to_le_bytes().as_slice(), chunk].concat();
        pages.push((13 + index, lval(&body)));
    }
    database(
        code,
        compressed,
        &[Some(descriptor(0, payload.len(), 13 << 8))],
        &pages,
    )
}
