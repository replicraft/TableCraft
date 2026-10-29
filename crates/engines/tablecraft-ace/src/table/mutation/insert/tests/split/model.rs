//! Literal source and complete-image expectations for public INSERT splitting.
use super::super::model as base;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;
const TABLE: usize = 10;
const ROOT: usize = 13;
const MAP_ROW: usize = 4027;

pub(in crate::table::mutation::insert::tests) struct Source {
    pub bytes: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
    pub(in crate::table::mutation::insert::tests) keys: Vec<(i32, u32)>,
    full: bool,
}

pub(in crate::table::mutation::insert::tests) fn record(id: i32, binary: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend(id.to_le_bytes());
    if let Some(binary) = binary {
        bytes.extend_from_slice(binary);
    }
    let end = bytes.len() as u16;
    bytes.extend(end.to_le_bytes());
    bytes.extend([6, 0, 1, 0, if binary.is_some() { 3 } else { 1 }]);
    bytes
}

fn inline(pages: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut row = vec![0; 69];
    for page in pages {
        assert!(page < 512);
        row[5 + page / 8] |= 1 << (page % 8);
    }
    row
}

fn install(bytes: &mut [u8], page: usize, data: &[u8]) {
    assert_eq!(data.len(), PAGE);
    bytes[page * PAGE..(page + 1) * PAGE].copy_from_slice(data);
}

pub(in crate::table::mutation::insert::tests) fn wire(value: i32, reference: u32) -> Vec<u8> {
    [
        vec![127],
        ((value as u32) ^ 0x80000000).to_be_bytes().to_vec(),
        reference.to_be_bytes().to_vec(),
    ]
    .concat()
}

pub(in crate::table::mutation::insert::tests) fn leaf(
    keys: &[(i32, u32)],
    prefix: usize,
    previous: u32,
    next: u32,
    residue: u8,
) -> Vec<u8> {
    let mut bytes = vec![residue; PAGE];
    bytes[..480].fill(0);
    bytes[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut bytes, 4, TABLE as u32);
    support::put32(&mut bytes, 12, previous);
    support::put32(&mut bytes, 16, next);
    support::put16(&mut bytes, 24, prefix as u16);
    let mut end = 0;
    for (position, (value, reference)) in keys.iter().enumerate() {
        let encoded = wire(*value, *reference);
        let shared = if position == 0 { 0 } else { prefix };
        let next = end + encoded.len() - shared;
        bytes[480 + end..480 + next].copy_from_slice(&encoded[shared..]);
        bytes[27 + next / 8] |= 1 << (next % 8);
        end = next;
    }
    support::put16(&mut bytes, 2, (3616 - end) as u16);
    bytes
}

pub(in crate::table::mutation::insert::tests) fn source(
    code: u8,
    aliases: bool,
    full: bool,
) -> Source {
    let mut bytes = base::source(code, aliases, false, None);
    bytes.resize(if full { 26 * PAGE } else { 16 * PAGE }, 0);
    let pages = if full {
        std::iter::once(12).chain(15..25).collect::<Vec<_>>()
    } else {
        vec![12, 15]
    };
    let per_page = if full { 41 } else { 255 };
    let values = (-226..0).chain(1..=225).collect::<Vec<i32>>();
    assert_eq!(values.len(), 451);
    let mut rows = Vec::new();
    let mut keys = Vec::new();
    for (position, subset) in values.chunks(per_page).enumerate() {
        let page = pages[position];
        let mut records = Vec::new();
        for (slot, id) in subset.iter().enumerate() {
            let ordinal = position * per_page + slot;
            let binary = if full {
                Some(vec![ordinal as u8; 84])
            } else if ordinal % 2 == 0 {
                None
            } else {
                Some(Vec::new())
            };
            records.push((0, record(*id, binary.as_deref())));
            rows.push(vec![
                Value::Int32(*id),
                binary.map_or(Value::Null, Value::Binary),
            ]);
            keys.push((*id, (page as u32) << 8 | slot as u32));
        }
        let used = records.iter().map(|(_, row)| row.len() + 2).sum::<usize>();
        let mut data = support::data_page(TABLE as u32, &records);
        support::put16(&mut data, 2, (4082 - used) as u16);
        if full {
            assert_eq!(records.len(), 41);
            assert!(records.iter().all(|(_, row)| row.len() == 97));
            assert_eq!(4082 - used, 23);
        } else {
            assert!(records.iter().all(|(_, row)| row.len() == 13));
        }
        install(&mut bytes, page, &data);
    }
    assert_eq!(keys.len(), 451);
    install(&mut bytes, ROOT, &leaf(&keys, 1, 0, 0, 0xa7));
    let hints = if full { vec![24] } else { vec![12, 15] };
    install(
        &mut bytes,
        11,
        &support::data_page(0, &[(0, inline(pages)), (0, inline(hints))]),
    );
    let eof = bytes.len() / PAGE;
    install(
        &mut bytes,
        1,
        &support::data_page(1, &[(0, inline(eof..512))]),
    );
    support::put32(&mut bytes, TABLE * PAGE + 16, 451);
    support::put32(&mut bytes, TABLE * PAGE + 67, 451);
    Source {
        bytes,
        rows,
        keys,
        full,
    }
}

pub(super) fn new_values(full: bool) -> Vec<Value> {
    vec![
        Value::Int32(226),
        Value::Binary(if full { vec![0xe1; 84] } else { Vec::new() }),
    ]
}

pub(in crate::table::mutation::insert::tests) fn expected(source: &Source) -> (Vec<u8>, u32) {
    let before = &source.bytes;
    let mut after = before.clone();
    let (reference, right, left) = if source.full {
        after.resize(29 * PAGE, 0);
        let mut data = support::data_page(TABLE as u32, &[(0, record(226, Some(&[0xe1; 84])))]);
        support::put16(&mut data, 2, 3983);
        install(&mut after, 26, &data);
        (26 << 8, 27, 28)
    } else {
        after.resize(18 * PAGE, 0);
        let data = &mut after[15 * PAGE..16 * PAGE];
        let end = u16::from_le_bytes(data[404..406].try_into().unwrap()) as usize;
        let row = record(226, Some(&[]));
        assert_eq!(row.len(), 13);
        let start = end - row.len();
        data[start..end].copy_from_slice(&row);
        support::put16(data, 406, start as u16);
        support::put16(data, 12, 197);
        support::put16(data, 2, (start - 14 - 197 * 2) as u16);
        ((15 << 8) | 196, 16, 17)
    };
    let mut keys = source.keys.clone();
    keys.push((226, reference));
    assert!(keys.windows(2).all(|pair| pair[0].0 < pair[1].0));
    let middle = keys.len() / 2;
    assert_eq!(middle, 226);
    assert_eq!(keys[middle - 1].0, -1);
    install(
        &mut after,
        left,
        &leaf(&keys[..middle], 0, 0, right as u32, 0),
    );
    install(
        &mut after,
        right,
        &leaf(&keys[middle..], 0, left as u32, 0, 0),
    );
    let mut root = before[ROOT * PAGE..(ROOT + 1) * PAGE].to_vec();
    root[0] = 3;
    root[26] = 1;
    support::put16(&mut root, 2, 3603);
    support::put32(&mut root, 20, right as u32);
    support::put16(&mut root, 24, 0);
    root[27..480].fill(0);
    let (value, row) = keys[middle - 1];
    let mut separator = wire(value, row);
    separator.extend((left as u32).to_be_bytes());
    assert_eq!(separator.len(), 13);
    root[480..493].copy_from_slice(&separator);
    root[28] |= 1 << 5;
    install(&mut after, ROOT, &root);
    for page in [right, left] {
        after[14 * PAGE + MAP_ROW + 5 + page / 8] |= 1 << (page % 8);
    }
    if source.full {
        after[11 * PAGE + MAP_ROW + 5 + 26 / 8] |= 1 << (26 % 8);
        let hints = 11 * PAGE + 3958 + 5;
        after[hints + 24 / 8] &= !(1 << (24 % 8));
        after[hints + 26 / 8] |= 1 << (26 % 8);
    }
    for page in before.len() / PAGE..after.len() / PAGE {
        after[PAGE + MAP_ROW + 5 + page / 8] &= !(1 << (page % 8));
    }
    support::put32(&mut after, TABLE * PAGE + 16, 452);
    support::put32(&mut after, TABLE * PAGE + 67, 452);
    (after, reference)
}

pub(super) fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    if actual != expected {
        let offset = actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
            .unwrap();
        panic!(
            "whole-image mismatch at {offset} (page {}, offset {}): actual {:02x}, expected {:02x}",
            offset / PAGE,
            offset % PAGE,
            actual[offset],
            expected[offset],
        );
    }
}
