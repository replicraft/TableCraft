//! Separate late-promotion and represented-peer literal complete images.
use super::model::{
    BITMAP_HEADER, DATA, EOF, GLOBAL_BITMAP, MAP_BYTES, MAPS, Model, PAGE, PER_PAGE, ROOTS, ROWS,
    TABLE, row_range,
};
use super::{
    Ace16WritePreview, CatalogScope, Cursor, EditError, IndexCheckStatus, Value, WriteBlocker,
    before, check_table, fixed_model, model, read_table, run, support,
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

const LATE_ROOT: usize = 89;
const EXISTING_PEER_BITMAP: usize = 520;
const INLINE_BASE: usize = 1;
const INLINE_BITS: usize = 5;
const ROOT_AFTER_USAGE: usize = 4;

fn source(late: bool, represented_peer: bool) -> Model {
    assert_ne!(late, represented_peer);
    let mut model = model::source(6);
    if late {
        let root = model.bytes[ROOTS[0] * PAGE..(ROOTS[0] + 1) * PAGE].to_vec();
        model::install(&mut model.bytes, LATE_ROOT, &root);
        let at = model::selected_usage_position(&model.bytes, 0) + ROOT_AFTER_USAGE;
        support::put32(&mut model.bytes, at, LATE_ROOT as u32);
        let mut inline = vec![0; MAP_BYTES];
        put(&mut inline, INLINE_BASE, LATE_ROOT, 4);
        inline[INLINE_BITS] = 1;
        model::install(
            &mut model.bytes,
            MAPS[0],
            &support::data_page(0, &[(0, inline)]),
        );
        assert_eq!(EOF - LATE_ROOT, 511);
        assert_eq!(EOF + 1 - LATE_ROOT, 512);
    }
    if represented_peer {
        model::install(&mut model.bytes, EXISTING_PEER_BITMAP, &bitmap(&[ROOTS[1]]));
        let mut indirect = vec![0; MAP_BYTES];
        indirect[0] = 1;
        put(&mut indirect, 1, EXISTING_PEER_BITMAP, 4);
        model::install(
            &mut model.bytes,
            MAPS[1],
            &support::data_page(0, &[(0, indirect)]),
        );
    }
    model
}

fn expected(model: &Model, late: bool, represented_peer: bool) -> Vec<u8> {
    assert_ne!(late, represented_peer);
    assert_eq!(model.bytes.len(), EOF * PAGE);
    let mut result = model.bytes.clone();
    let (row, _) = row_range(&result, DATA[0], 0);
    result[row + SELECTED_FIELD..row + SELECTED_FIELD + 4].copy_from_slice(&i32::MAX.to_le_bytes());
    let roots = [if late { LATE_ROOT } else { ROOTS[0] }, ROOTS[1]];
    // The represented first child is reserved before the second child makes
    // promotion necessary. Its metadata is therefore AFTER both index children.
    let schedule = if late {
        [(600, Some(602), 601), (603, Some(604), 605)]
    } else {
        [(600, Some(601), 602), (603, None, 604)]
    };
    let mut appended = Vec::new();
    for (physical, (&root, &(right, metadata, left))) in roots.iter().zip(&schedule).enumerate() {
        let entries = keys(physical == 1);
        let left_entries = &entries[..450];
        let right_entries = &entries[450..];
        appended.push((right, leaf(left, 0, right_entries)));
        appended.push((left, leaf(0, right, left_entries)));
        if let Some(metadata) = metadata {
            appended.push((metadata, bitmap(&[root, right, left])));
            let mut row = vec![0; MAP_BYTES];
            row[0] = 1;
            put(&mut row, 1, metadata, 4);
            let (start, end) = model.selected_map_ranges[physical];
            result[start..end].copy_from_slice(&row);
            assert_eq!(result[end - 1], 0);
        } else {
            for page in [right, left] {
                let at = EXISTING_PEER_BITMAP * PAGE + BITMAP_HEADER + page / 8;
                assert_eq!(model.bytes[at] & (1 << (page % 8)), 0);
                result[at] |= 1 << (page % 8);
            }
        }
        let original = &model.bytes[root * PAGE..(root + 1) * PAGE];
        let mut header = original.to_vec();
        header[0] = 3;
        header[HEIGHT] = 1;
        put(&mut header, PREVIOUS, 0, 4);
        put(&mut header, NEXT, 0, 4);
        put(&mut header, TAIL, right, 4);
        let mut bound = left_entries.last().unwrap().clone();
        bound.extend((left as u32).to_be_bytes());
        let (page, end) = packet(&header, &[bound]);
        assert_eq!(&page[end..], &original[end..]);
        result[root * PAGE..(root + 1) * PAGE].copy_from_slice(&page);
    }
    appended.sort_by_key(|(number, _)| *number);
    let count = appended.len();
    for (page, bytes) in appended {
        assert_eq!(result.len(), page * PAGE);
        result.extend(bytes);
        let at = GLOBAL_BITMAP * PAGE + BITMAP_HEADER + page / 8;
        assert_ne!(model.bytes[at] & (1 << (page % 8)), 0);
        result[at] &= !(1 << (page % 8));
    }
    assert_eq!(count, if late { 6 } else { 5 });
    assert_eq!(result.len(), (EOF + count) * PAGE);
    let global_byte = GLOBAL_BITMAP * PAGE + BITMAP_HEADER + EOF / 8;
    assert_eq!(result[global_byte], if late { 0xc0 } else { 0xe0 });
    result
}

fn verify(model: &Model, result: &Ace16WritePreview, late: bool, represented_peer: bool) {
    assert_eq!(result.changed_rows(), 1);
    assert_eq!(
        result.candidate_bytes(),
        expected(model, late, represented_peer)
    );
    let table = read_table(
        &mut Cursor::new(result.candidate_bytes()),
        CatalogScope::new(1),
        "Items",
        &mut fixed_model::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), ROWS);
    for (row, values) in table.rows().iter().enumerate() {
        assert_eq!(values[0], Value::Int32(row as i32 + 1));
        assert_eq!(values[1], Value::Int32(if row == 0 { i32::MAX } else { 0 }));
    }
    let report = check_table(
        &mut Cursor::new(result.candidate_bytes()),
        CatalogScope::new(1),
        "Items",
        &mut fixed_model::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.indexes().len(), 4);
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
}

#[test]
fn late_promotion_absorbs_earlier_inline_edits_and_every_reserved_index_child() {
    let model = source(true, false);
    before(&model, 4);
    let original = model.bytes.clone();
    let accepted = run(&model).unwrap();
    verify(&model, &accepted, true, false);
    assert_eq!(model.bytes, original);
    let (_, end) = model.selected_map_ranges[0];
    // right600 initially uses old inline bit511 (the final row byte). It must
    // not survive as a stale patch after the whole row becomes indirect.
    assert_eq!(accepted.candidate_bytes()[end - 1], 0);
}

#[test]
fn a_promotion_operation_checks_represented_peer_authority_as_well() {
    let model = source(false, true);
    before(&model, 4);
    verify(&model, &run(&model).unwrap(), false, true);
    let mut aliased = model.clone();
    support::put32(
        &mut aliased.bytes,
        model::OTHER * PAGE + model::TABLE_HINT,
        (MAPS[1] as u32) << 8,
    );
    before(&aliased, 4);
    let original = aliased.bytes.clone();
    assert_eq!(
        run(&aliased).err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
    assert_eq!(aliased.bytes, original);
}
