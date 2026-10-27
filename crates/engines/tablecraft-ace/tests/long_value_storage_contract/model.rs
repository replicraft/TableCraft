use super::{base, support};
use std::io::Cursor;
use tablecraft_ace::{InspectError, TableLongValueCheck, check_table_long_values, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

pub const PAGE_BYTES: usize = 4096;
pub const DECLARATION: usize = 10 * PAGE_BYTES + 129;

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 10_000, 5_000_000)
}

fn row(id: i32, present: bool) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend_from_slice(&id.to_le_bytes());
    // Single external value: two raw UTF16 bytes at LVAL page13, row0.
    bytes.extend_from_slice(&[2, 0, 0, 0x40, 0, 13, 0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&[18, 0, 6, 0, 1, 0]);
    bytes.push(if present { 3 } else { 1 });
    bytes
}

fn framed(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = support::data_page(owner, rows);
    let body_bytes: usize = rows.iter().map(|(_, body)| body.len()).sum();
    let free = PAGE_BYTES - 14 - 2 * rows.len() - body_bytes;
    support::put16(&mut bytes, 2, free as u16);
    bytes
}

pub fn image(roots: usize, present: bool) -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let mut definition = base::definition(
        &[("Id", 4, 0, 4), ("Memo", 12, 0, 0)],
        roots as u32,
        11 << 8,
    );
    support::put16(&mut definition, 43, 1);
    definition[103] = 2;
    // An unaligned ten-byte entry after the validated names; FFFF ends it.
    definition[129..141].copy_from_slice(&[1, 0, 2, 11, 0, 0, 3, 11, 0, 0, 255, 255]);
    bytes[10 * PAGE_BYTES..11 * PAGE_BYTES].copy_from_slice(&definition);

    let table = vec![0, 0, 0, 0, 0, 0, 0x10];
    let column = vec![0, 0, 0, 0, 0, 0, 0x20];
    bytes[11 * PAGE_BYTES..12 * PAGE_BYTES].copy_from_slice(&framed(
        0,
        &[
            (0, table.clone()),
            (0, table),
            (0, column.clone()),
            (0, column),
        ],
    ));
    let rows = (0..roots)
        .map(|index| (0, row(index as i32 + 1, present)))
        .collect::<Vec<_>>();
    bytes[12 * PAGE_BYTES..13 * PAGE_BYTES].copy_from_slice(&framed(10, &rows));

    // The orphan is deliberately too short to contain a chain pointer.
    // Ownership permits opaque unreachable rows; only live descriptors decode.
    let mut lval = framed(0, &[(0, vec![0x41, 0]), (0, vec![0xff, 0xff, 0xff])]);
    lval[4..8].copy_from_slice(b"LVAL");
    bytes[13 * PAGE_BYTES..14 * PAGE_BYTES].copy_from_slice(&lval);
    let mut global = vec![0; 69];
    for number in 14..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&framed(1, &[(0, global)]));
    bytes
}

pub fn overlapping_columns_image() -> Vec<u8> {
    let mut bytes = image(1, true);
    let mut definition = base::definition(
        &[("Id", 4, 0, 4), ("Memo", 12, 0, 0), ("Ole", 11, 0, 0)],
        1,
        11 << 8,
    );
    support::put16(&mut definition, 43, 2);
    support::put16(&mut definition, 95, 0);
    support::put16(&mut definition, 120, 1);
    definition[103] = 2;
    definition[128] = 2;
    definition[162..184].copy_from_slice(&[
        1, 0, 2, 11, 0, 0, 3, 11, 0, 0, 2, 0, 4, 11, 0, 0, 5, 11, 0, 0, 255, 255,
    ]);
    bytes[10 * PAGE_BYTES..11 * PAGE_BYTES].copy_from_slice(&definition);

    let table = vec![0, 0, 0, 0, 0, 0, 0x10];
    let column = vec![0, 0, 0, 0, 0, 0, 0x20];
    bytes[11 * PAGE_BYTES..12 * PAGE_BYTES].copy_from_slice(&framed(
        0,
        &[
            (0, table.clone()),
            (0, table),
            (0, column.clone()),
            (0, column.clone()),
            (0, column.clone()),
            (0, column),
        ],
    ));
    let descriptor = [2, 0, 0, 0x40, 0, 13, 0, 0, 0, 0, 0, 0];
    let mut record = vec![3, 0, 1, 0, 0, 0];
    record.extend_from_slice(&descriptor);
    record.extend_from_slice(&descriptor);
    record.extend_from_slice(&[30, 0, 18, 0, 6, 0, 2, 0, 7]);
    bytes[12 * PAGE_BYTES..13 * PAGE_BYTES].copy_from_slice(&framed(10, &[(0, record)]));
    bytes
}

pub fn set_member(bytes: &mut [u8], page: usize, slot: usize, number: usize, set: bool) {
    let at = page * PAGE_BYTES + 14 + 2 * slot;
    let start = usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) & 0x1fff);
    let byte = page * PAGE_BYTES + start + 5 + number / 8;
    let mask = 1 << (number % 8);
    if set {
        bytes[byte] |= mask;
    } else {
        bytes[byte] &= !mask;
    }
}

pub fn read(bytes: &[u8]) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}

pub fn check(bytes: &[u8]) -> Result<TableLongValueCheck, InspectError> {
    check_table_long_values(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
}
