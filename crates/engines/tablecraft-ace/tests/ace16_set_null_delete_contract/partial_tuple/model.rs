//! Two genuine physical indexes separate single-field DELETE from tuple UPDATE.
use super::super::{model, role_boundary, support};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const COUNTER: usize = 12;
const PHYSICAL: usize = 52;
const LOGICAL: usize = 28;
const ENTRY: usize = 480;
const TUPLE_ROOT: usize = 25;
const TUPLE_MAP: usize = 26;

pub fn source(code: u8, original_partial: bool) -> model::Model {
    let mut source = role_boundary::source(code);
    source.bytes.resize(27 * PAGE, 0);
    let retained = if original_partial {
        Value::Null
    } else {
        Value::Int32(200)
    };
    source.rows[0][1][2] = retained.clone();
    source.rows[2][0][2] = retained;
    for target in [0, 2] {
        write_records(&mut source, target);
    }
    items_definition(&mut source);
    other_definition(&mut source);
    write_leaf(&mut source.bytes, &source.rows[0], &[], 0, 14, &[1]);
    write_leaf(
        &mut source.bytes,
        &source.rows[0],
        &[],
        0,
        TUPLE_ROOT,
        &[2, 1],
    );
    write_leaf(&mut source.bytes, &source.rows[2], &[], 2, 24, &[2, 1]);

    let mut owned = vec![0; 9];
    owned[5 + TUPLE_ROOT / 8] |= 1 << (TUPLE_ROOT % 8);
    source.bytes[TUPLE_MAP * PAGE..(TUPLE_MAP + 1) * PAGE]
        .copy_from_slice(&support::data_page(0, &[(0, owned)]));
    let start = u16::from_le_bytes(source.bytes[PAGE + 14..PAGE + 16].try_into().unwrap());
    let global = PAGE + usize::from(start & 0x1fff);
    for page in [TUPLE_ROOT, TUPLE_MAP] {
        source.bytes[global + 5 + page / 8] &= !(1 << (page % 8));
    }
    let records = vec![
        (
            0,
            relationship("First", "Parents", "Items", 0x2000, 1, 0, "Score"),
        ),
        (
            0,
            relationship("Second", "Items", "Other", 0x100, 2, 0, "K2"),
        ),
        (
            0,
            relationship("Second", "Items", "Other", 0x100, 2, 1, "Score"),
        ),
    ];
    support::put32(&mut source.bytes, 7 * PAGE + 16, 3);
    source.bytes[9 * PAGE..10 * PAGE].copy_from_slice(&support::data_page(7, &records));
    source
}

fn items_definition(source: &mut model::Model) {
    let at = model::definition(0) * PAGE;
    let original = source.bytes[at..at + PAGE].to_vec();
    let old_physical = source.logical_start - PHYSICAL;
    let mut page = vec![0; PAGE];
    page[..63].copy_from_slice(&original[..63]);
    support::put32(&mut page, 47, 4);
    support::put32(&mut page, 51, 2);
    page[63..63 + COUNTER].copy_from_slice(&original[63..63 + COUNTER]);
    support::put32(&mut page, 63 + COUNTER + 4, 4);
    let columns = &original[63 + COUNTER..old_physical];
    let physical = 63 + 2 * COUNTER + columns.len();
    page[63 + 2 * COUNTER..physical].copy_from_slice(columns);
    page[physical..physical + PHYSICAL]
        .copy_from_slice(&original[old_physical..old_physical + PHYSICAL]);
    page[physical + 46] = 0x80;
    let tuple = physical + PHYSICAL;
    page[tuple..tuple + PHYSICAL].copy_from_slice(&original[old_physical..old_physical + PHYSICAL]);
    support::put16(&mut page, tuple + 4, 2);
    support::put16(&mut page, tuple + 7, 1);
    support::put32(&mut page, tuple + 34, (TUPLE_MAP << 8) as u32);
    support::put32(&mut page, tuple + 38, TUPLE_ROOT as u32);
    page[tuple + 46] = 0x81;
    let logical = tuple + PHYSICAL;
    write_logicals(
        &mut page,
        logical,
        &[
            ("IncomingScore", declaration(0, 2, 1, 1, 0, 2)),
            ("OutgoingTuple", declaration(1, 1, 2, 0, 1, 0)),
            ("TupleUnique", declaration(1, 0, 0, 0, 0, 0)),
            ("ScoreAlias", declaration(0, 0, 0, 0, 0, 0)),
        ],
    );
    source.bytes[at..at + PAGE].copy_from_slice(&page);
}

fn other_definition(source: &mut model::Model) {
    let base = model::definition(2) * PAGE;
    let physical = base + source.logical_start - PHYSICAL;
    support::put16(&mut source.bytes, physical + 4, 2);
    support::put16(&mut source.bytes, physical + 7, 1);
    let mut page = source.bytes[base..base + PAGE].to_vec();
    write_logicals(
        &mut page,
        source.logical_start,
        &[
            ("IncomingTuple", declaration(0, 2, 0, 1, 1, 0)),
            ("TupleAlias", declaration(0, 0, 0, 0, 0, 0)),
        ],
    );
    source.bytes[base..base + PAGE].copy_from_slice(&page);
}

fn declaration(
    physical: u32,
    role: u8,
    peer: usize,
    peer_index: u32,
    update: u8,
    delete: u8,
) -> [u8; LOGICAL] {
    let mut block = [0; LOGICAL];
    support::put32(&mut block, 8, physical);
    if role != 0 {
        block[12] = role;
        support::put32(&mut block, 13, peer_index);
        support::put32(&mut block, 17, model::definition(peer) as u32);
        block[21] = update;
        block[22] = delete;
        block[23] = 2;
    }
    block
}

fn write_logicals(page: &mut [u8], start: usize, entries: &[(&str, [u8; LOGICAL])]) {
    let mut names = start + entries.len() * LOGICAL;
    for (number, (name, block)) in entries.iter().enumerate() {
        let at = start + number * LOGICAL;
        page[at..at + LOGICAL].copy_from_slice(block);
        support::put32(page, at + 4, number as u32);
        let bytes: Vec<_> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        support::put16(page, names, bytes.len() as u16);
        names += 2;
        page[names..names + bytes.len()].copy_from_slice(&bytes);
        names += bytes.len();
    }
}

fn write_records(source: &mut model::Model, target: usize) {
    source.records[target] = source.rows[target]
        .iter()
        .map(|values| {
            let mut record = vec![3, 0];
            let mut mask = 0;
            for (column, value) in values.iter().enumerate() {
                if let Value::Int32(value) = value {
                    record.extend(value.to_le_bytes());
                    mask |= 1 << column;
                } else {
                    assert_eq!(value, &Value::Null);
                    record.extend([0xa5; 4]);
                }
            }
            record.push(mask);
            record
        })
        .collect();
    let records: Vec<_> = source.records[target]
        .iter()
        .cloned()
        .map(|record| (0, record))
        .collect();
    let at = model::data(target) * PAGE;
    source.bytes[at..at + PAGE].copy_from_slice(&support::data_page(
        model::definition(target) as u32,
        &records,
    ));
}

fn relationship(
    name: &str,
    parent: &str,
    child: &str,
    flags: u32,
    width: u32,
    ordinal: u32,
    column: &str,
) -> Vec<u8> {
    let mut record = vec![8, 0];
    for value in [flags, width, ordinal] {
        record.extend(value.to_le_bytes());
    }
    let mut offsets = vec![record.len() as u16];
    for text in [name, parent, child, column, column] {
        record.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        offsets.push(record.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        record.extend(offset.to_le_bytes());
    }
    record.extend(5_u16.to_le_bytes());
    record.push(255);
    record
}

fn write_leaf(
    bytes: &mut [u8],
    rows: &[Vec<Value>],
    deleted: &[usize],
    target: usize,
    root: usize,
    columns: &[usize],
) {
    let start = root * PAGE;
    bytes[start..start + 2].copy_from_slice(&[4, 1]);
    support::put32(bytes, start + 4, model::definition(target) as u32);
    bytes[start + 27..start + ENTRY].fill(0);
    let mut entries = Vec::new();
    for (row, values) in rows.iter().enumerate() {
        if deleted.contains(&row) {
            continue;
        }
        let mut key = Vec::new();
        for &column in columns {
            if let Value::Int32(value) = values[column] {
                key.push(127);
                key.extend(((value as u32) ^ 0x80000000).to_be_bytes());
            } else {
                assert_eq!(values[column], Value::Null);
                key.push(0);
            }
        }
        key.extend((((model::data(target) as u32) << 8) | row as u32).to_be_bytes());
        entries.push(key);
    }
    entries.sort();
    let mut end = 0;
    for entry in entries {
        bytes[start + ENTRY + end..start + ENTRY + end + entry.len()].copy_from_slice(&entry);
        end += entry.len();
        bytes[start + 27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(bytes, start + 2, (PAGE - ENTRY - end) as u16);
}

pub fn expected(source: &model::Model) -> model::Expected {
    let mut bytes = source.bytes.clone();
    let mut rows = source.rows.clone();
    rows[1].remove(1);
    rows[0][1][1] = Value::Null;
    rows[2][0][1] = Value::Null;
    support::put32(&mut bytes, model::definition(1) * PAGE + 16, 2);
    let directory = model::data(1) * PAGE + 16;
    let slot = u16::from_le_bytes(bytes[directory..directory + 2].try_into().unwrap());
    support::put16(&mut bytes, directory, slot | 0xc000);
    for (target, row) in [(0, 1), (2, 0)] {
        let directory = model::data(target) * PAGE + 14 + row * 2;
        let slot = u16::from_le_bytes(bytes[directory..directory + 2].try_into().unwrap());
        let mask = model::data(target) * PAGE + usize::from(slot & 0x1fff) + 14;
        bytes[mask] &= !2;
    }
    write_leaf(&mut bytes, &rows[0], &[], 0, 14, &[1]);
    write_leaf(&mut bytes, &rows[0], &[], 0, TUPLE_ROOT, &[2, 1]);
    write_leaf(&mut bytes, &source.rows[1], &[1], 1, 19, &[1]);
    write_leaf(&mut bytes, &rows[2], &[], 2, 24, &[2, 1]);
    model::Expected {
        bytes,
        rows,
        removed: 1,
    }
}
