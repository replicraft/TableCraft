use super::{numeric_fixture, support};

const PAGE_BYTES: usize = 4096;
const ENTRY_START: usize = 480;
const ROOT_PAGE: usize = 14;
const LEAVES: [u32; 3] = [15, 16, 17];

pub fn entry(kind: u8, key: i64, row: usize, descending: bool) -> Vec<u8> {
    let mut bytes = vec![if descending { 0x80 } else { 0x7f }];
    let value = match kind {
        1 => vec![if key == -1 { 0 } else { 255 }],
        2 => vec![key as u8],
        3 => ((key as u16) ^ 0x8000).to_be_bytes().to_vec(),
        4 => ((key as u32) ^ 0x80000000).to_be_bytes().to_vec(),
        6..=8 => numeric_fixture::floating_key(kind, key),
        5 | 0x13 => ((key as u64) ^ 0x8000000000000000).to_be_bytes().to_vec(),
        _ => panic!("authored numeric kind"),
    };
    bytes.extend(
        value
            .into_iter()
            .map(|byte| if descending { !byte } else { byte }),
    );
    bytes.extend(((12_u32 << 8) | row as u32).to_be_bytes());
    bytes
}

fn page(height: u8, entries: &[Vec<u8>], previous: u32, next: u32, tail: u32) -> Vec<u8> {
    let mut bytes = vec![0xa7; PAGE_BYTES];
    bytes[..ENTRY_START].fill(0);
    bytes[0] = if height == 0 { 4 } else { 3 };
    bytes[1] = 1;
    bytes[26] = height;
    support::put32(&mut bytes, 4, 10);
    support::put32(&mut bytes, 12, previous);
    support::put32(&mut bytes, 16, next);
    support::put32(&mut bytes, 20, tail);
    let mut end = 0;
    for entry in entries {
        bytes[ENTRY_START + end..ENTRY_START + end + entry.len()].copy_from_slice(entry);
        end += entry.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut bytes, 2, (PAGE_BYTES - ENTRY_START - end) as u16);
    bytes
}

pub fn values(kind: u8) -> [i64; 8] {
    std::array::from_fn(|row| match kind {
        1 => {
            if row % 2 == 0 {
                -1
            } else {
                0
            }
        }
        6 => i64::from((row as f32 + 1.0).to_bits()),
        7 | 8 => (row as f64 + 1.0).to_bits() as i64,
        _ => row as i64 + 1,
    })
}

pub fn database(code: u8, kind: u8, descending: bool, bounded: bool) -> (Vec<u8>, Vec<Vec<usize>>) {
    let values = values(kind);
    let rows = values
        .iter()
        .enumerate()
        .map(|(row, key)| vec![Some(row as i64 + 1), Some(*key)])
        .collect::<Vec<_>>();
    let mut bytes = numeric_fixture::database(code, &[4, kind], &[(1, descending)], &rows, 0);
    bytes.resize(18 * PAGE_BYTES, 0);
    bytes[13 * PAGE_BYTES..14 * PAGE_BYTES].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xc0, 0x03])],
    ));
    let mut order = (0..values.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        let comparison =
            numeric_fixture::compare_cell(kind, Some(values[*left]), Some(values[*right]));
        (if descending {
            comparison.reverse()
        } else {
            comparison
        })
        .then(left.cmp(right))
    });
    let partitions = vec![
        order[..2].to_vec(),
        order[2..5].to_vec(),
        order[5..].to_vec(),
    ];
    let mut nodes = Vec::new();
    for (position, partition) in partitions.iter().enumerate() {
        let entries = partition
            .iter()
            .map(|&row| entry(kind, values[row], row, descending))
            .collect::<Vec<_>>();
        let previous = if position == 0 {
            0
        } else {
            LEAVES[position - 1]
        };
        let next = LEAVES.get(position + 1).copied().unwrap_or(0);
        let number = LEAVES[position] as usize;
        bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES]
            .copy_from_slice(&page(0, &entries, previous, next, 0));
        if bounded || position < partitions.len() - 1 {
            let mut maximum = entries.last().unwrap().clone();
            maximum.extend(LEAVES[position].to_be_bytes());
            nodes.push(maximum);
        }
    }
    let tail = if bounded { 0 } else { LEAVES[2] };
    bytes[ROOT_PAGE * PAGE_BYTES..(ROOT_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&page(1, &nodes, 0, 0, tail));
    (bytes, partitions)
}

pub fn raw_record(bytes: &[u8], row: usize) -> Vec<u8> {
    let page = &bytes[12 * PAGE_BYTES..13 * PAGE_BYTES];
    let word = |slot| {
        u16::from_le_bytes(page[14 + slot * 2..16 + slot * 2].try_into().unwrap()) as usize & 0x1fff
    };
    let start = word(row);
    let end = if row == 0 { PAGE_BYTES } else { word(row - 1) };
    page[start..end].to_vec()
}

pub fn capacity_database() -> Vec<u8> {
    const CHILDREN: usize = 302;
    const FIRST_LEAF: usize = 20;
    const FIRST_KEY: i32 = 0x1234_0000;
    const DATA_PAGES: [usize; 3] = [12, 18, 19];
    const ROWS_PER_PAGE: usize = 220;

    let (mut bytes, _) = database(6, 4, false, true);
    bytes.resize((FIRST_LEAF + CHILDREN) * PAGE_BYTES, 0);
    support::put32(&mut bytes, 10 * PAGE_BYTES + 16, (CHILDREN * 2) as u32);
    let mut data_map = vec![0; 5 + 3];
    for number in DATA_PAGES {
        data_map[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[11 * PAGE_BYTES..12 * PAGE_BYTES]
        .copy_from_slice(&support::data_page(1, &[(0, data_map)]));
    let mut index_map = vec![0; 5 + (FIRST_LEAF + CHILDREN).div_ceil(8)];
    for number in std::iter::once(ROOT_PAGE).chain(FIRST_LEAF..FIRST_LEAF + CHILDREN) {
        index_map[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[13 * PAGE_BYTES..14 * PAGE_BYTES]
        .copy_from_slice(&support::data_page(1, &[(0, index_map)]));
    let keys = (0..CHILDREN * 2)
        .map(|row| {
            if row == 0 {
                i32::MIN
            } else {
                FIRST_KEY + row as i32
            }
        })
        .collect::<Vec<_>>();
    let mut references = Vec::new();
    for (position, chunk) in keys.chunks(ROWS_PER_PAGE).enumerate() {
        let records = chunk
            .iter()
            .enumerate()
            .map(|(slot, key)| {
                let ordinal = position * ROWS_PER_PAGE + slot;
                let mut payload = vec![2, 0];
                payload.extend((ordinal as i32 + 1).to_le_bytes());
                payload.extend(key.to_le_bytes());
                payload.push(3);
                references.push(((DATA_PAGES[position] as u32) << 8) | slot as u32);
                (0, payload)
            })
            .collect::<Vec<_>>();
        let number = DATA_PAGES[position];
        bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(10, &records));
    }
    let mut separators = Vec::new();
    for child in 0..CHILDREN {
        let mut entries = Vec::new();
        for ordinal in child * 2..child * 2 + 2 {
            let mut key = vec![0x7f];
            key.extend(((keys[ordinal] as u32) ^ 0x80000000).to_be_bytes());
            key.extend(references[ordinal].to_be_bytes());
            entries.push(key);
        }
        let number = FIRST_LEAF + child;
        let previous = if child == 0 { 0 } else { number as u32 - 1 };
        let next = if child + 1 == CHILDREN {
            0
        } else {
            number as u32 + 1
        };
        bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES]
            .copy_from_slice(&page(0, &entries, previous, next, 0));
        let mut bound = entries.last().unwrap().clone();
        bound.extend((number as u32).to_be_bytes());
        separators.push(bound);
    }
    // The old full bounds share marker+two high key bytes (three bytes).
    // Removing first leaf's maximum leaves MIN and reduces the common prefix
    // to the marker alone: 13 + 301*12 = 3625 bytes > 3616 available.
    let mut root = vec![0xa7; PAGE_BYTES];
    root[..ENTRY_START].fill(0);
    root[0..2].copy_from_slice(&[3, 1]);
    root[26] = 1;
    support::put32(&mut root, 4, 10);
    support::put16(&mut root, 24, 3);
    let mut end = 0;
    for (position, separator) in separators.iter().enumerate() {
        let shared = if position == 0 { 0 } else { 3 };
        assert_eq!(&separator[..shared], &separators[0][..shared]);
        let encoded = &separator[shared..];
        root[ENTRY_START + end..ENTRY_START + end + encoded.len()].copy_from_slice(encoded);
        end += encoded.len();
        root[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut root, 2, (PAGE_BYTES - ENTRY_START - end) as u16);
    bytes[ROOT_PAGE * PAGE_BYTES..(ROOT_PAGE + 1) * PAGE_BYTES].copy_from_slice(&root);
    bytes
}
