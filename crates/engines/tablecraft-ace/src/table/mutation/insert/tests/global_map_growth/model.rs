//! Literal allocation events and global defaults, independent of append State.
use crate::table::mutation::insert::tests::{
    long_value_inserts::{Family, external, initialization::model as long_rows},
    multiple_indexes::model as frames,
    split::model as split,
};
use tablecraft_core::values::Value;
use tablecraft_testkit::access as raw;
use tablecraft_testkit::access::multiple_indexes as source_frames;

const PAGE: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub enum Phase {
    Data,
    Index,
    LongValue,
    Container,
    OwnedBitmap,
}

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub old_rows: Vec<Vec<Value>>,
    pub final_rows: Vec<Vec<Value>>,
    pub values: Vec<Value>,
    pub aliases: usize,
    pub global_bitmap: Option<usize>,
}

fn row_start(bytes: &[u8], page: usize, slot: usize) -> usize {
    let offset = page * PAGE + 14 + slot * 2;
    page * PAGE
        + usize::from(u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) & 8191)
}

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    assert_eq!(page.len(), PAGE);
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn inline(base: u32, members: &[u32]) -> Vec<u8> {
    let mut row = vec![0; 69];
    row[1..5].copy_from_slice(&base.to_le_bytes());
    for number in members {
        let relative = number.checked_sub(base).unwrap();
        assert!(relative < 512);
        row[5 + relative as usize / 8] |= 1 << (relative % 8);
    }
    row
}

fn replace_map(bytes: &mut [u8], page: usize, slot: usize, replacement: &[u8]) {
    assert_eq!(replacement.len(), 69);
    let start = row_start(bytes, page, slot);
    bytes[start..start + 69].copy_from_slice(replacement);
}

fn source_global(bytes: &mut Vec<u8>, eof: usize, hole: bool) {
    assert!(matches!(eof, 511 | 512));
    bytes.resize(eof * PAGE, 0);
    let mut free = (eof as u32..512).collect::<Vec<_>>();
    if hole {
        free.push(30);
    }
    replace_map(bytes, 1, 0, &inline(0, &free));
}

fn global_after(before: &[u8], after: &mut [u8], bitmap: Option<usize>) {
    after[PAGE..2 * PAGE].copy_from_slice(&before[PAGE..2 * PAGE]);
    if let Some(number) = bitmap {
        assert_eq!(number, 513);
        let mut record = vec![0; 69];
        record[0] = 1;
        record[1..5].copy_from_slice(&(number as u32).to_le_bytes());
        replace_map(after, 1, 0, &record);
        let mut page = vec![255; PAGE];
        page[..4].copy_from_slice(&[5, 1, 0, 0]);
        // The pinned reference writer's fill ends at byte4092, exclusively.
        // This is an observed initialization policy, not a reader span limit.
        page[4092..].fill(0);
        for allocated in 0..after.len() / PAGE {
            page[4 + allocated / 8] &= !(1 << (allocated % 8));
        }
        install(after, number, &page);
    } else {
        let start = row_start(after, 1, 0);
        for number in before.len() / PAGE..after.len() / PAGE {
            assert!(number < 512);
            after[start + 5 + number / 8] &= !(1 << (number % 8));
        }
    }
}

fn long_page(payload: &[u8]) -> Vec<u8> {
    let mut page = raw::data_page(0, &[(0, payload.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    page[2..4].copy_from_slice(&((PAGE - 16 - payload.len()) as u16).to_le_bytes());
    page
}

pub fn long_value(code: u8, eof: usize, hole: bool, phase: Phase) -> Case {
    let container = matches!(phase, Phase::Container);
    let indirect = matches!(phase, Phase::OwnedBitmap);
    let mut source = external::original(code, Family::Ole);
    install(&mut source.before, 16, &long_page(&vec![0x5a; 4076]));
    if container {
        source.before[10 * PAGE + 500..10 * PAGE + 508].fill(0);
    }
    if indirect {
        let mut owned = vec![0; 69];
        owned[0] = 1;
        replace_map(&mut source.before, 11, 2, &owned);
        replace_map(&mut source.before, 11, 3, &inline(0, &[]));
    }
    source_global(&mut source.before, eof, hole);
    let body = if container {
        if eof == 511 { 512 } else { 514 }
    } else {
        eof
    };
    let bitmap = (container || indirect || eof == 512).then_some(513);
    let owned_bitmap = indirect.then_some(if eof == 511 { 512 } else { 514 });
    let end = (body + 1)
        .max(bitmap.map_or(0, |number| number + 1))
        .max(owned_bitmap.map_or(0, |number| number + 1));
    let value = vec![0x92; 65];
    let mut descriptor = vec![65, 0, 0, 64];
    descriptor.extend(((body as u32) << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut expected = frames::expected(
        &source,
        &[source_frames::Row {
            id: 10,
            group: Some(100),
            binary: Some(descriptor),
        }],
    );
    expected[11 * PAGE..12 * PAGE].copy_from_slice(&source.before[11 * PAGE..12 * PAGE]);
    expected.resize(end * PAGE, 0);
    install(&mut expected, body, &long_page(&value));

    if container {
        let owned = inline(512, &[body as u32]);
        let candidate = inline(512, &[body as u32]);
        let mut page = raw::data_page(0, &[(0, owned), (0, candidate)]);
        page[2..4].copy_from_slice(&3940_u16.to_le_bytes());
        install(&mut expected, eof, &page);
        expected[10 * PAGE + 500..10 * PAGE + 504]
            .copy_from_slice(&((eof as u32) << 8).to_le_bytes());
        expected[10 * PAGE + 504..10 * PAGE + 508]
            .copy_from_slice(&(((eof as u32) << 8) | 1).to_le_bytes());
    } else if let Some(number) = owned_bitmap {
        let mut owned = vec![0; 69];
        owned[0] = 1;
        owned[1..5].copy_from_slice(&(number as u32).to_le_bytes());
        replace_map(&mut expected, 11, 2, &owned);
        let mut page = vec![0; PAGE];
        page[..4].copy_from_slice(&[5, 1, 0, 0]);
        page[4 + body / 8] |= 1 << (body % 8);
        install(&mut expected, number, &page);
        replace_map(
            &mut expected,
            11,
            3,
            &inline(if body == 511 { 0 } else { 512 }, &[body as u32]),
        );
    } else {
        let owned_base = if body == 511 { 0 } else { 16 };
        let candidate_base = if body == 511 { 0 } else { 512 };
        replace_map(
            &mut expected,
            11,
            2,
            &inline(owned_base, &[16, body as u32]),
        );
        replace_map(
            &mut expected,
            11,
            3,
            &inline(candidate_base, &[body as u32]),
        );
    }
    global_after(&source.before, &mut expected, bitmap);
    let stored_value = Value::Binary(value);
    Case {
        before: source.before,
        expected,
        old_rows: long_rows::rows(Family::Ole, None),
        final_rows: long_rows::rows(Family::Ole, Some(&stored_value)),
        values: vec![Value::Int32(10), Value::Int32(100), stored_value],
        aliases: 4,
        global_bitmap: bitmap,
    }
}

pub fn ordinary(code: u8, eof: usize, hole: bool, full: bool) -> Case {
    let mut source = split::source(code, true, full);
    source_global(&mut source.bytes, eof, hole);
    let mut expected = source.bytes.clone();
    let (reference, right, left, bitmap) = if full {
        if eof == 511 {
            (511 << 8, 512, 514, Some(513))
        } else {
            (512 << 8, 514, 515, Some(513))
        }
    } else if eof == 511 {
        ((15 << 8) | 196, 511, 512, Some(513))
    } else {
        ((15 << 8) | 196, 512, 514, Some(513))
    };
    let end = (left + 1).max(bitmap.map_or(0, |number| number + 1));
    expected.resize(end * PAGE, 0);
    if full {
        let number = (reference >> 8) as usize;
        let mut page = raw::data_page(10, &[(0, split::record(226, Some(&[0xe1; 84])))]);
        page[2..4].copy_from_slice(&3983_u16.to_le_bytes());
        install(&mut expected, number, &page);
        let mut members = vec![12];
        members.extend(15..25);
        members.push(number as u32);
        let base = if number == 511 { 0 } else { 8 };
        replace_map(&mut expected, 11, 0, &inline(base, &members));
        replace_map(
            &mut expected,
            11,
            1,
            &inline(if number == 511 { 0 } else { 512 }, &[number as u32]),
        );
    } else {
        let page = &mut expected[15 * PAGE..16 * PAGE];
        let end = usize::from(u16::from_le_bytes(page[404..406].try_into().unwrap()));
        let row = split::record(226, Some(&[]));
        let start = end - row.len();
        page[start..end].copy_from_slice(&row);
        page[406..408].copy_from_slice(&(start as u16).to_le_bytes());
        page[12..14].copy_from_slice(&197_u16.to_le_bytes());
        page[2..4].copy_from_slice(&((start - 14 - 394) as u16).to_le_bytes());
    }

    let mut keys = source.keys.clone();
    keys.push((226, reference));
    let middle = keys.len() / 2;
    assert_eq!(middle, 226);
    install(
        &mut expected,
        left,
        &split::leaf(&keys[..middle], 0, 0, right as u32, 0),
    );
    install(
        &mut expected,
        right,
        &split::leaf(&keys[middle..], 0, left as u32, 0, 0),
    );
    let mut root = source.bytes[13 * PAGE..14 * PAGE].to_vec();
    root[0] = 3;
    root[26] = 1;
    root[2..4].copy_from_slice(&3603_u16.to_le_bytes());
    root[20..24].copy_from_slice(&(right as u32).to_le_bytes());
    root[24..26].fill(0);
    root[27..480].fill(0);
    let mut separator = split::wire(keys[middle - 1].0, keys[middle - 1].1);
    separator.extend((left as u32).to_be_bytes());
    root[480..493].copy_from_slice(&separator);
    root[28] |= 1 << 5;
    install(&mut expected, 13, &root);
    replace_map(
        &mut expected,
        14,
        0,
        &inline(8, &[13, right as u32, left as u32]),
    );
    global_after(&source.bytes, &mut expected, bitmap);
    expected[10 * PAGE + 16..10 * PAGE + 20].copy_from_slice(&452_u32.to_le_bytes());
    expected[10 * PAGE + 67..10 * PAGE + 71].copy_from_slice(&452_u32.to_le_bytes());
    let values = vec![
        Value::Int32(226),
        Value::Binary(if full { vec![0xe1; 84] } else { Vec::new() }),
    ];
    let mut final_rows = source.rows.clone();
    final_rows.push(values.clone());
    Case {
        before: source.bytes,
        expected,
        old_rows: source.rows,
        final_rows,
        values,
        aliases: 2,
        global_bitmap: bitmap,
    }
}

pub fn case(code: u8, phase: Phase, eof: usize, hole: bool) -> Case {
    match phase {
        Phase::Data => ordinary(code, eof, hole, true),
        Phase::Index => ordinary(code, eof, hole, false),
        Phase::LongValue | Phase::Container | Phase::OwnedBitmap => {
            long_value(code, eof, hole, phase)
        }
    }
}
