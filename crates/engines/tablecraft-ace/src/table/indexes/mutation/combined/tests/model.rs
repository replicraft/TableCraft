//! Existing independent field/key fixtures, with explicit NULL/delete goldens.
use crate::test_support::base;
use tablecraft_testkit::access as support;

#[allow(dead_code)]
mod numeric_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/composite_indexes/fixture.rs"
    ));
}
#[allow(dead_code)]
mod guid_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/guid_indexes/fixture.rs"
    ));
}
#[allow(dead_code)]
mod decimal_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/decimal_indexes/fixture.rs"
    ));
}

pub const PAGE: usize = 4096;

pub fn source(code: u8, kind: u8, descending: bool, flags: u8) -> Vec<u8> {
    let mut bytes = match kind {
        15 => guid_fixture::database(
            code,
            descending,
            &[Some([1; 16]), Some([2; 16]), Some([3; 16])],
            0,
        ),
        16 => {
            decimal_fixture::database_for(code, descending, 28, 2, &[Some(1), Some(2), Some(3)], 0)
        }
        _ => numeric_fixture::database(
            code,
            &[4, kind],
            &[(1, descending)],
            &[
                vec![Some(1), Some(1)],
                vec![Some(2), Some(2)],
                vec![Some(3), Some(3)],
            ],
            0,
        ),
    };
    bytes[10 * PAGE + 189] = flags;
    support::put32(&mut bytes, 10 * PAGE + 67, 3);
    geometry(&mut bytes, 12);
    bytes
}

fn geometry(bytes: &mut [u8], number: usize) {
    let base = number * PAGE;
    let count = u16::from_le_bytes(bytes[base + 12..base + 14].try_into().unwrap()) as usize;
    let end = u16::from_le_bytes(
        bytes[base + 12 + count * 2..base + 14 + count * 2]
            .try_into()
            .unwrap(),
    ) as usize;
    support::put16(bytes, base + 2, (end - 14 - count * 2) as u16);
}

pub fn key(kind: u8, value: Option<u8>, reference: u32, descending: bool) -> Vec<u8> {
    let mut bytes = if let Some(value) = value {
        let mut bytes = vec![127];
        match kind {
            2 => bytes.push(value),
            3 => bytes.extend([128, value]),
            4 => bytes.extend([128, 0, 0, value]),
            5 | 19 => bytes.extend([128, 0, 0, 0, 0, 0, 0, value]),
            15 => {
                bytes.extend([value; 8]);
                bytes.push(9);
                bytes.extend([value; 8]);
                bytes.push(8);
            }
            16 => {
                bytes.push(255);
                bytes.extend([0; 15]);
                bytes.push(value);
            }
            _ => unreachable!(),
        }
        if descending {
            for (position, byte) in bytes.iter_mut().enumerate() {
                if kind != 15 || position != 9 {
                    *byte = !*byte;
                }
            }
        }
        bytes
    } else {
        vec![if descending { 255 } else { 0 }]
    };
    bytes.extend(reference.to_be_bytes());
    bytes
}

pub fn expected_root(original: &[u8], mut entries: Vec<Vec<u8>>) -> Vec<u8> {
    entries.sort();
    let mut page = original.to_vec();
    page[27..480].fill(0);
    support::put16(&mut page, 24, 0);
    let mut end = 0;
    for entry in entries {
        page[480 + end..480 + end + entry.len()].copy_from_slice(&entry);
        end += entry.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut page, 2, (3616 - end) as u16);
    page
}

pub fn apply_intents(bytes: &mut [u8], deleted: &[usize], nulled: &[usize]) {
    let base = 12 * PAGE;
    for slot in deleted {
        let at = base + 14 + slot * 2;
        let word = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
        support::put16(bytes, at, word | 0xc000);
    }
    for slot in nulled {
        let end = if *slot == 0 {
            PAGE
        } else {
            u16::from_le_bytes(
                bytes[base + 12 + slot * 2..base + 14 + slot * 2]
                    .try_into()
                    .unwrap(),
            ) as usize
        };
        bytes[base + end - 1] &= !2;
    }
    support::put32(bytes, 10 * PAGE + 16, (3 - deleted.len()) as u32);
}

pub fn pressure_source() -> Vec<u8> {
    let mut bytes = source(6, 4, false, 0x80);
    bytes.resize(17 * PAGE, 0);
    support::put32(&mut bytes, 10 * PAGE + 16, 451);
    support::put32(&mut bytes, 10 * PAGE + 67, 451);
    for (page, range) in [(12, 0..255), (16, 255..451)] {
        let rows = range
            .map(|position: i32| {
                let mut row = vec![2, 0];
                row.extend((position + 1).to_le_bytes());
                row.extend((position + 1).to_le_bytes());
                row.push(3);
                (0, row)
            })
            .collect::<Vec<_>>();
        bytes[page * PAGE..(page + 1) * PAGE].copy_from_slice(&support::data_page(10, &rows));
        geometry(&mut bytes, page);
    }
    let mut owned = vec![0; 69];
    owned[5 + 12 / 8] |= 1 << (12 % 8);
    owned[5 + 16 / 8] |= 1 << (16 % 8);
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(0, &[(0, owned)]));
    let mut root = vec![0xa7; PAGE];
    root[..480].fill(0);
    root[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut root, 4, 10);
    support::put16(&mut root, 24, 1);
    let mut end = 0;
    for position in 0..451_u32 {
        let page = if position < 255 { 12 } else { 16 };
        let slot = if position < 255 {
            position
        } else {
            position - 255
        };
        let mut wire = vec![127];
        wire.extend(((position + 1) ^ 0x80000000).to_be_bytes());
        wire.extend(((page << 8) | slot).to_be_bytes());
        let suffix = &wire[usize::from(position != 0)..];
        root[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        root[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, 3609);
    support::put16(&mut root, 2, 7);
    bytes[14 * PAGE..15 * PAGE].copy_from_slice(&root);
    bytes
}
