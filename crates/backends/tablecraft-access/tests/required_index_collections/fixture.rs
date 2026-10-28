//! Prospective four real parent rows with duplicate visible scalar values.
//! Four actual logical roots; duplicate Id values cannot identify memberships.
use crate::{base, modern_fixture, support};

pub const PAGE: usize = 4096;
const OWNER: usize = 10;
const PARENT_DATA: usize = 12;
pub const INDEX_ROOT: usize = 24;
const INDEX_MAP: usize = 25;
pub const HIDDEN_TARGET: usize = 26;

pub fn source(code: u8, descending: bool, forwarded: bool) -> Vec<u8> {
    with_data(code, descending, forwarded, None)
}

pub fn with_data(code: u8, descending: bool, forwarded: bool, file_data: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = file_data.map_or_else(
        || modern_fixture::database(code),
        |data| modern_fixture::with_first_data(code, data),
    );
    bytes.resize((if forwarded { 27 } else { 26 }) * PAGE, 0);
    let parents = [
        (30_i32, Some(10_i32)),
        (10, None),
        (10, Some(20)),
        (20, Some(30)),
    ];
    let rows = parents
        .iter()
        .map(|(identity, key)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(identity.to_le_bytes());
            row.extend(key.unwrap_or(777).to_le_bytes());
            row.push(if key.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[PARENT_DATA * PAGE..(PARENT_DATA + 1) * PAGE]
        .copy_from_slice(&support::data_page(OWNER as u32, &rows));
    let definition = &mut bytes[OWNER * PAGE..(OWNER + 1) * PAGE];
    definition.copy_within(63..131, 75);
    definition[63..75].fill(0);
    support::put32(definition, 16, 4);
    support::put32(definition, 47, 1);
    support::put32(definition, 51, 1);
    // Three distinct scalar keys, four full key/ROW entries. This historical
    // distinct counter is not normalized to the current live row count.
    support::put32(definition, 67, 3);
    definition[143..195].fill(0);
    for slot in 0..10 {
        support::put16(definition, 147 + slot * 3, 0xffff);
    }
    support::put16(definition, 147, 0);
    definition[149] = u8::from(!descending);
    support::put32(definition, 177, (INDEX_MAP as u32) << 8);
    support::put32(definition, 181, INDEX_ROOT as u32);
    definition[189] = 0x80; // Ordinary nonunique index; duplicate key10 is real.
    definition[195..223].fill(0);
    let label = "IdOrder"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    support::put16(definition, 223, label.len() as u16);
    definition[225..225 + label.len()].copy_from_slice(&label);

    let mut entries = parents
        .iter()
        .enumerate()
        .map(|(slot, (identity, _))| {
            let mut wire = vec![if descending { 128 } else { 127 }];
            let key = ((*identity as u32) ^ 0x80000000).to_be_bytes();
            wire.extend(key.map(|byte| if descending { !byte } else { byte }));
            wire.extend(((PARENT_DATA as u32) << 8 | slot as u32).to_be_bytes());
            wire
        })
        .collect::<Vec<_>>();
    entries.sort();
    let mut leaf = vec![0; PAGE];
    leaf[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut leaf, 4, OWNER as u32);
    support::put16(&mut leaf, 2, 3580);
    for (position, wire) in entries.iter().enumerate() {
        let start = 480 + position * 9;
        leaf[start..start + 9].copy_from_slice(wire);
        let end = (position + 1) * 9;
        leaf[27 + end / 8] |= 1 << (end % 8);
    }
    bytes[INDEX_ROOT * PAGE..(INDEX_ROOT + 1) * PAGE].copy_from_slice(&leaf);
    let mut map = vec![0; 69];
    map[5 + INDEX_ROOT / 8] |= 1 << (INDEX_ROOT % 8);
    bytes[INDEX_MAP * PAGE..(INDEX_MAP + 1) * PAGE]
        .copy_from_slice(&support::data_page(1, &[(0, map)]));

    if forwarded {
        let logical = &rows[2].1;
        bytes[HIDDEN_TARGET * PAGE..(HIDDEN_TARGET + 1) * PAGE].copy_from_slice(
            &support::data_page(OWNER as u32, &[(0x8000, logical.clone())]),
        );
        let at = PARENT_DATA * PAGE;
        let word = u16::from_le_bytes(bytes[at + 18..at + 20].try_into().unwrap());
        let start = at + usize::from(word);
        bytes[start..start + 11].fill(0);
        support::put32(&mut bytes, start, (HIDDEN_TARGET as u32) << 8);
        support::put16(&mut bytes, at + 18, word | 0x4000);
        let mut owned = vec![0; 69];
        for page in [PARENT_DATA, HIDDEN_TARGET] {
            owned[5 + page / 8] |= 1 << (page % 8);
        }
        bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(1, &[(0, owned)]));
    }
    bytes
}

pub fn replace_index(bytes: &mut [u8], entries: &[(i32, u32)]) {
    let mut entries = entries
        .iter()
        .map(|(identity, reference)| {
            let mut wire = vec![127];
            wire.extend(((*identity as u32) ^ 0x80000000).to_be_bytes());
            wire.extend(reference.to_be_bytes());
            wire
        })
        .collect::<Vec<_>>();
    entries.sort();
    let page = &mut bytes[INDEX_ROOT * PAGE..(INDEX_ROOT + 1) * PAGE];
    page.fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(page, 4, OWNER as u32);
    support::put16(page, 2, (3616 - entries.len() * 9) as u16);
    for (position, wire) in entries.iter().enumerate() {
        let start = 480 + position * 9;
        page[start..start + 9].copy_from_slice(wire);
        let end = (position + 1) * 9;
        page[27 + end / 8] |= 1 << (end % 8);
    }
}

pub fn multivalue(code: u8, descending: bool, forwarded: bool) -> Vec<u8> {
    let mut bytes = source(code, descending, forwarded);
    bytes[17 * PAGE..18 * PAGE].copy_from_slice(&base::definition(
        &[("Value", 4, 0, 4)],
        0,
        18 << 8,
    ));
    let fields = [("Owner", 4, 0, 4), ("Value", 4, 4, 4), ("ChildID", 4, 8, 4)];
    let mut flat = base::definition(&fields, 3, 21 << 8);
    flat[63 + 16] = 8;
    flat[63 + 50 + 15] = 7;
    bytes[20 * PAGE..21 * PAGE].copy_from_slice(&flat);
    let rows = [
        (10_i32, Some(100_i32), 1_i32),
        (10, None, 2),
        (20, Some(0), 3),
    ]
    .into_iter()
    .map(|(owner, value, identity)| {
        let mut row = 3_u16.to_le_bytes().to_vec();
        row.extend(owner.to_le_bytes());
        row.extend(value.unwrap_or(777).to_le_bytes());
        row.extend(identity.to_le_bytes());
        row.push(if value.is_some() { 7 } else { 5 });
        (0, row)
    })
    .collect::<Vec<_>>();
    bytes[22 * PAGE..23 * PAGE].copy_from_slice(&support::data_page(20, &rows));
    bytes
}
