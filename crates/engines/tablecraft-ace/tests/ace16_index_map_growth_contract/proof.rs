//! Literal map-promotion images. No production serializer or candidate is read.
use super::model::{
    BITMAP_HEADER, DATA, DIRECTORY, EOF, GLOBAL_BITMAP, MAP_BYTES, MAPS, Model, PAGE, PER_PAGE,
    ROOTS, ROWS, TABLE, number, row_range,
};

const ENTRY: usize = 480;
const MASK: usize = 27;
const FREE: usize = 2;
const PREFIX: usize = 24;
const HEIGHT: usize = 26;
const PREVIOUS: usize = 12;
const NEXT: usize = 16;
const TAIL: usize = 20;
const OWNER: usize = 4;
const ROW_BYTES: usize = 4;
const CHILD_BYTES: usize = 4;
const SELECTED_FIELD: usize = 6;
const CAPACITY: usize = PAGE - ENTRY;

fn put(bytes: &mut [u8], offset: usize, value: usize, width: usize) {
    for position in 0..width {
        bytes[offset + position] = (value >> (position * 8)) as u8;
    }
}

fn reference(row: usize) -> u32 {
    ((DATA[row / PER_PAGE] as u32) << 8) | (row % PER_PAGE) as u32
}

fn keys(descending: bool) -> Vec<Vec<u8>> {
    let mut entries = Vec::new();
    for row in 0..ROWS {
        let value = if row == 0 { i32::MAX } else { 0 };
        let encoded = ((value as u32) ^ 0x8000_0000).to_be_bytes();
        let mut entry = vec![if descending { 128 } else { 127 }];
        for byte in encoded {
            entry.push(if descending { !byte } else { byte });
        }
        entry.extend(reference(row).to_be_bytes());
        entries.push(entry);
    }
    entries.sort();
    entries
}

fn common(entries: &[Vec<u8>]) -> usize {
    let mut prefix = entries[0].len();
    for entry in &entries[1..] {
        prefix = prefix.min(entry.len());
        for position in 0..prefix {
            if entry[position] != entries[0][position] {
                prefix = position;
                break;
            }
        }
    }
    prefix
}

fn packet(initial: &[u8], entries: &[Vec<u8>]) -> (Vec<u8>, usize) {
    let total: usize = entries.iter().map(Vec::len).sum();
    let prefix = if total <= CAPACITY {
        0
    } else {
        common(entries)
    };
    let mut result = initial.to_vec();
    result[MASK..ENTRY].fill(0);
    put(&mut result, PREFIX, prefix, 2);
    let mut used = 0;
    for (position, entry) in entries.iter().enumerate() {
        let suffix = if position == 0 {
            entry.as_slice()
        } else {
            &entry[prefix..]
        };
        assert_eq!(&entry[..prefix], &entries[0][..prefix]);
        assert!(used + suffix.len() <= CAPACITY);
        result[ENTRY + used..ENTRY + used + suffix.len()].copy_from_slice(suffix);
        used += suffix.len();
        result[MASK + used / 8] |= 1 << (used % 8);
    }
    put(&mut result, FREE, CAPACITY - used, 2);
    (result, ENTRY + used)
}

fn leaf(previous: usize, next: usize, entries: &[Vec<u8>]) -> Vec<u8> {
    let mut header = vec![0; PAGE];
    header[..2].copy_from_slice(&[4, 1]);
    put(&mut header, OWNER, TABLE, 4);
    put(&mut header, PREVIOUS, previous, 4);
    put(&mut header, NEXT, next, 4);
    let (page, end) = packet(&header, entries);
    assert!(page[end..].iter().all(|&byte| byte == 0));
    page
}

fn bitmap(members: &[usize]) -> Vec<u8> {
    let mut result = vec![0; PAGE];
    result[..BITMAP_HEADER].copy_from_slice(&[5, 1, 0, 0]);
    for &page in members {
        result[BITMAP_HEADER + page / 8] |= 1 << (page % 8);
    }
    result
}

pub fn expected(model: &Model) -> Vec<u8> {
    assert_eq!(model.bytes.len(), EOF * PAGE);
    let mut result = model.bytes.clone();
    let (start, end) = row_range(&result, DATA[0], 0);
    assert_eq!(end - start, 11);
    assert_eq!(&result[start..start + 6], &[2, 0, 1, 0, 0, 0]);
    assert_eq!(
        &result[start + SELECTED_FIELD..start + SELECTED_FIELD + 4],
        &[0; 4]
    );
    result[start + SELECTED_FIELD..start + SELECTED_FIELD + 4]
        .copy_from_slice(&i32::MAX.to_le_bytes());
    for (physical, &root) in ROOTS.iter().enumerate() {
        let right = EOF + physical * 3;
        let metadata = right + 1;
        let left = right + 2;
        assert_eq!(result.len(), right * PAGE);
        let entries = keys(physical == 1);
        let middle = entries.len() / 2;
        let left_keys = &entries[..middle];
        let right_keys = &entries[middle..];
        assert_eq!(middle, 450);
        let right_page = leaf(left, 0, right_keys);
        let left_page = leaf(0, right, left_keys);
        result.extend(right_page);
        result.extend(bitmap(&[root, right, left]));
        result.extend(left_page);
        // The bitmap is globally allocated metadata; it is never an index member.
        let bit_offset = metadata * PAGE + BITMAP_HEADER + metadata / 8;
        assert_eq!(result[bit_offset] & (1 << (metadata % 8)), 0);
        let mut promoted = vec![0; MAP_BYTES];
        promoted[0] = 1;
        put(&mut promoted, 1, metadata, 4);
        let (map_start, map_end) = model.selected_map_ranges[physical];
        assert_eq!(map_end - map_start, MAP_BYTES);
        result[map_start..map_end].copy_from_slice(&promoted);
        for allocated in [right, metadata, left] {
            let offset = GLOBAL_BITMAP * PAGE + BITMAP_HEADER + allocated / 8;
            assert_ne!(model.bytes[offset] & (1 << (allocated % 8)), 0);
            result[offset] &= !(1 << (allocated % 8));
        }
        let original = &model.bytes[root * PAGE..(root + 1) * PAGE];
        let mut root_header = original.to_vec();
        root_header[0] = 3;
        root_header[HEIGHT] = 1;
        put(&mut root_header, PREVIOUS, 0, 4);
        put(&mut root_header, NEXT, 0, 4);
        put(&mut root_header, TAIL, right, 4);
        let mut bound = left_keys.last().unwrap().clone();
        bound.extend((left as u32).to_be_bytes());
        assert_eq!(bound.len(), 1 + 4 + ROW_BYTES + CHILD_BYTES);
        let (root_page, active_end) = packet(&root_header, &[bound]);
        assert_eq!(&root_page[active_end..], &original[active_end..]);
        result[root * PAGE..(root + 1) * PAGE].copy_from_slice(&root_page);
    }
    assert_eq!(result.len(), (EOF + 6) * PAGE);
    let global_cell = GLOBAL_BITMAP * PAGE + BITMAP_HEADER + EOF / 8;
    assert_eq!(model.bytes[global_cell], 0xff);
    assert_eq!(result[global_cell], 0xc0);
    // Row directories, metadata container free words and other rows stay exact.
    for &container in &MAPS {
        assert_eq!(
            &result[container * PAGE..container * PAGE + DIRECTORY + 2],
            &model.bytes[container * PAGE..container * PAGE + DIRECTORY + 2],
        );
    }
    assert_eq!(number(&result, TABLE * PAGE + 16, 4), ROWS);
    result
}
