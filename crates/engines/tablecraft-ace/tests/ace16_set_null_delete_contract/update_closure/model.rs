//! Independent fixed-row masks, reciprocal declarations and complete leaf images.
use super::super::{model, role_boundary, support};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const LOGICAL: usize = 28;
const PHYSICAL: usize = 52;
const ENTRY: usize = 480;

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Chain,
    Tuple,
    Diamond,
    Cycle,
    Restrict,
    RestrictDiamond,
    Required,
}

pub fn source(code: u8, shape: Shape) -> model::Model {
    let mut source = role_boundary::source(code);
    if matches!(shape, Shape::Tuple) {
        source.tuple = true;
        source.rows[0][3][2] = Value::Int32(300);
        source.rows[1][1][2] = Value::Int32(200);
        source.rows[2][0][2] = Value::Int32(200);
        for target in 0..3 {
            records(&mut source, target);
            let at = model::definition(target) * PAGE + source.logical_start - PHYSICAL;
            support::put16(&mut source.bytes, at + 4, 2);
            support::put16(&mut source.bytes, at + 7, 1);
            leaf(&mut source.bytes, &source.rows[target], target, true);
        }
    }
    let action = u8::from(!matches!(shape, Shape::Restrict | Shape::RestrictDiamond));
    // Every tuple alias uses the same physical order [K2, Score]. The catalog
    // relationship order is also explicit, rather than inferred from values.
    let mut items = vec![link(2, 1, 1, 0, 2), link(1, 2, 0, action, 0), ordinary()];
    let mut parents = vec![primary(), link(1, 0, 0, 0, 2), ordinary()];
    let mut other = vec![link(2, 0, 1, action, 0)];
    let mut relations = vec![
        ("First", 1, 0, 0x2000),
        ("Second", 0, 2, u32::from(action) * 0x100),
    ];
    if matches!(shape, Shape::Diamond | Shape::RestrictDiamond) {
        parents[2] = link(1, 2, 1, 0, 2);
        other.push(link(2, 1, 2, 0, 2));
        relations.push(("DirectNull", 1, 2, 0x2000));
    }
    if matches!(shape, Shape::Cycle) {
        // The reciprocal UPDATE edge makes Other a parent of every non-NULL
        // Items key. Keep the original key 20 at ROW 0 and supply real keys
        // 10/30, with complete DATA, counter and leaf records for all three.
        source.rows[2].extend([
            vec![Value::Int32(2), Value::Int32(10), Value::Int32(100)],
            vec![Value::Int32(3), Value::Int32(30), Value::Int32(300)],
        ]);
        records(&mut source, 2);
        let definition = model::definition(2) * PAGE;
        support::put32(&mut source.bytes, definition + 16, 3);
        support::put32(&mut source.bytes, definition + 67, 3);
        leaf(&mut source.bytes, &source.rows[2], 2, false);
        items.push(link(2, 2, 1, 1, 0));
        other.push(link(1, 0, 3, 1, 0));
        other.push(ordinary());
        let physical = model::definition(2) * PAGE + source.logical_start - PHYSICAL;
        source.bytes[physical + 46] = 0x81;
        relations.push(("Return", 2, 0, 0x100));
    }
    if matches!(shape, Shape::Required) {
        let physical = model::definition(2) * PAGE + source.logical_start - PHYSICAL;
        source.bytes[physical + 46] = 0x88;
    }
    for (target, entries) in [items, parents, other].into_iter().enumerate() {
        logical(&mut source, target, &entries);
    }
    let columns: &[&str] = if source.tuple {
        &["K2", "Score"]
    } else {
        &["Score"]
    };
    let relations = relations
        .into_iter()
        .flat_map(|(name, parent, child, flags)| {
            columns.iter().enumerate().map(move |(ordinal, column)| {
                (
                    0,
                    relationship(name, parent, child, flags, columns.len(), ordinal, column),
                )
            })
        })
        .collect::<Vec<_>>();
    support::put32(&mut source.bytes, 7 * PAGE + 16, relations.len() as u32);
    source.bytes[9 * PAGE..10 * PAGE].copy_from_slice(&support::data_page(7, &relations));
    source
}

fn records(source: &mut model::Model, target: usize) {
    source.records[target] = source.rows[target]
        .iter()
        .map(|values| {
            let mut body = vec![3, 0];
            let mut mask = 0;
            for (column, value) in values.iter().enumerate() {
                match value {
                    Value::Int32(value) => {
                        body.extend(value.to_le_bytes());
                        mask |= 1 << column;
                    }
                    Value::Null => body.extend([0xa5; 4]),
                    _ => unreachable!(),
                }
            }
            body.push(mask);
            body
        })
        .collect();
    let records = source.records[target]
        .iter()
        .cloned()
        .map(|body| (0, body))
        .collect::<Vec<_>>();
    let at = model::data(target) * PAGE;
    source.bytes[at..at + PAGE].copy_from_slice(&support::data_page(
        model::definition(target) as u32,
        &records,
    ));
}

fn ordinary() -> [u8; LOGICAL] {
    [0; LOGICAL]
}

fn primary() -> [u8; LOGICAL] {
    let mut entry = ordinary();
    entry[23] = 1;
    entry
}

fn link(role: u8, peer: usize, index: u32, update: u8, delete: u8) -> [u8; LOGICAL] {
    let mut entry = ordinary();
    entry[12] = role;
    support::put32(&mut entry, 13, index);
    support::put32(&mut entry, 17, model::definition(peer) as u32);
    entry[21] = update;
    entry[22] = delete;
    entry[23] = 2;
    entry
}

fn logical(source: &mut model::Model, target: usize, entries: &[[u8; LOGICAL]]) {
    let base = model::definition(target) * PAGE;
    support::put32(&mut source.bytes, base + 47, entries.len() as u32);
    let mut at = base + source.logical_start;
    for (number, entry) in entries.iter().enumerate() {
        source.bytes[at..at + LOGICAL].copy_from_slice(entry);
        support::put32(&mut source.bytes, at + 4, number as u32);
        at += LOGICAL;
    }
    for number in 0..entries.len() {
        let text = format!("Alias{number}");
        let text = text
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(&mut source.bytes, at, text.len() as u16);
        at += 2;
        source.bytes[at..at + text.len()].copy_from_slice(&text);
        at += text.len();
    }
}

fn relationship(
    name: &str,
    parent: usize,
    child: usize,
    flags: u32,
    width: usize,
    ordinal: usize,
    column: &str,
) -> Vec<u8> {
    let mut body = vec![8, 0];
    for value in [flags, width as u32, ordinal as u32] {
        body.extend(value.to_le_bytes());
    }
    let mut offsets = vec![body.len() as u16];
    for text in [
        name,
        model::table(parent),
        model::table(child),
        column,
        column,
    ] {
        body.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        offsets.push(body.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        body.extend(offset.to_le_bytes());
    }
    body.extend(5_u16.to_le_bytes());
    body.push(255);
    body
}

fn leaf(bytes: &mut [u8], rows: &[Vec<Value>], target: usize, tuple: bool) {
    let start = model::root(target) * PAGE;
    bytes[start + 27..start + ENTRY].fill(0);
    support::put16(bytes, start + 24, 0);
    let columns: &[usize] = if tuple { &[2, 1] } else { &[1] };
    let mut entries = Vec::new();
    for (ordinal, values) in rows.iter().enumerate() {
        let mut key = Vec::new();
        for &column in columns {
            match values[column] {
                Value::Null => key.push(0),
                Value::Int32(value) => {
                    key.push(127);
                    key.extend(((value as u32) ^ 0x80000000).to_be_bytes());
                }
                _ => unreachable!(),
            }
        }
        key.extend((((model::data(target) as u32) << 8) | ordinal as u32).to_be_bytes());
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

pub fn expected(
    source: &model::Model,
    deleted: [&[usize]; 3],
    nulls: [&[usize]; 3],
) -> model::Expected {
    let mut result = model::expected(source, deleted, &[]);
    for (target, nulled) in nulls.into_iter().enumerate() {
        if nulled.is_empty() {
            continue;
        }
        // Logical ROW references remain source ordinals even after deletion.
        let mut indexed = source.rows[target].clone();
        let columns: &[usize] = if source.tuple { &[2, 1] } else { &[1] };
        for &row in nulled {
            assert!(!deleted[target].contains(&row));
            for &column in columns {
                indexed[row][column] = Value::Null;
            }
            let directory = model::data(target) * PAGE + 14 + row * 2;
            let slot =
                u16::from_le_bytes(source.bytes[directory..directory + 2].try_into().unwrap())
                    as usize
                    & 0x1fff;
            let mask = model::data(target) * PAGE + slot + source.records[target][row].len() - 1;
            result.bytes[mask] &= if source.tuple { !6 } else { !2 };
        }
        result.rows[target] = indexed
            .iter()
            .enumerate()
            .filter(|(row, _)| !deleted[target].contains(row))
            .map(|(_, values)| values.clone())
            .collect();
        // Build entries from source ordinals and retain their original references.
        let start = model::root(target) * PAGE;
        result.bytes[start + 27..start + ENTRY].fill(0);
        let mut entries = Vec::new();
        for (row, values) in indexed.iter().enumerate() {
            if deleted[target].contains(&row) {
                continue;
            }
            let mut key = Vec::new();
            for &column in columns {
                match values[column] {
                    Value::Null => key.push(0),
                    Value::Int32(value) => {
                        key.push(127);
                        key.extend(((value as u32) ^ 0x80000000).to_be_bytes());
                    }
                    _ => unreachable!(),
                }
            }
            key.extend((((model::data(target) as u32) << 8) | row as u32).to_be_bytes());
            entries.push(key);
        }
        entries.sort();
        let mut end = 0;
        for entry in entries {
            result.bytes[start + ENTRY + end..start + ENTRY + end + entry.len()]
                .copy_from_slice(&entry);
            end += entry.len();
            result.bytes[start + 27 + end / 8] |= 1 << (end % 8);
        }
        support::put16(&mut result.bytes, start + 2, (PAGE - ENTRY - end) as u16);
    }
    result
}
