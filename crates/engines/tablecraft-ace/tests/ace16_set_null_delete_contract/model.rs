//! Literal source and full expected image; no mutation codec/planner is imported.
use super::{cascade_model, restrict_model, support, tuple};
use std::io::Cursor;
use tablecraft_ace::{
    Ace16DeletePreview, IndexCheckStatus, check_table, check_table_constraints, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

pub use cascade_model::{before, budget, capture, data, definition, requests, root, run, table};
pub use restrict_model::Model;

const PAGE_BYTES: usize = 4096;
const TABLE_ROWS: usize = 16;
const LOGICAL_BYTES: usize = 28;
const DELETE_ACTION: usize = 22;
const SLOT_START: usize = 14;
const INDEX_START: usize = 480;
const INDEX_BITMAP: usize = 27;
const PHYSICAL_BYTES: usize = 52;
const PHYSICAL_FLAGS: usize = 46;

pub fn source(code: u8, composite: bool, second_cascade: bool) -> Model {
    let mut model = if composite {
        tuple::database()
    } else {
        cascade_model::database(code, second_cascade)
    };
    model.bytes[20] = code;

    let mut relationships = restrict_model::relationship("First", 0x2000, "Items", composite);
    relationships.extend(restrict_model::relationship(
        "Second",
        if second_cascade { 0x1000 } else { 0 },
        "Other",
        composite,
    ));
    if composite {
        let (pairs, remainder) = relationships.as_chunks_mut::<2>();
        assert!(remainder.is_empty());
        for pair in pairs {
            pair.swap(0, 1);
            for (ordinal, record) in pair.iter_mut().enumerate() {
                record[10..14].copy_from_slice(&(ordinal as u32).to_le_bytes());
            }
        }
    }
    support::put32(
        &mut model.bytes,
        7 * PAGE_BYTES + TABLE_ROWS,
        relationships.len() as u32,
    );
    let records: Vec<_> = relationships
        .into_iter()
        .map(|record| (0, record))
        .collect();
    model.bytes[9 * PAGE_BYTES..10 * PAGE_BYTES].copy_from_slice(&support::data_page(7, &records));
    for (target, logical, action) in [
        (0, 0, 2),
        (1, 1, 2),
        (1, 2, u8::from(second_cascade)),
        (2, 0, u8::from(second_cascade)),
    ] {
        let at = definition(target) * PAGE_BYTES + model.logical_start + logical * LOGICAL_BYTES;
        model.bytes[at + DELETE_ACTION] = action;
    }
    model
}

pub struct Expected {
    pub bytes: Vec<u8>,
    pub rows: [Vec<Vec<Value>>; 3],
    pub removed: usize,
}

pub fn with_ignored_child(mut model: Model) -> Model {
    let physical = definition(0) * PAGE_BYTES + model.logical_start - PHYSICAL_BYTES;
    model.bytes[physical + PHYSICAL_FLAGS] |= 0x02;
    let mut original = model.bytes.clone();
    write_leaf(&mut original, &model, 0, &[], &[]);
    model.bytes = original;
    model
}

pub fn expected(model: &Model, deleted: [&[usize]; 3], nulled: &[usize]) -> Expected {
    let mut bytes = model.bytes.clone();
    let rows = std::array::from_fn(|target| {
        model.rows[target]
            .iter()
            .enumerate()
            .filter(|(row, _)| !deleted[target].contains(row))
            .map(|(row, values)| {
                let mut values = values.clone();
                if target == 0 && nulled.contains(&row) {
                    values[1] = Value::Null;
                    if model.tuple {
                        values[2] = Value::Null;
                    }
                }
                values
            })
            .collect()
    });

    for (target, deleted) in deleted.into_iter().enumerate() {
        let count = model.rows[target].len() - deleted.len();
        support::put32(
            &mut bytes,
            definition(target) * PAGE_BYTES + TABLE_ROWS,
            count as u32,
        );
        for row in 0..model.records[target].len() {
            let at = data(target) * PAGE_BYTES + SLOT_START + row * 2;
            let slot = u16::from_le_bytes(model.bytes[at..at + 2].try_into().unwrap());
            if deleted.contains(&row) {
                support::put16(&mut bytes, at, slot | 0xc000);
            } else if target == 0 && nulled.contains(&row) {
                // SET NULL changes only presence bits; opaque fixed bytes survive.
                let record = data(target) * PAGE_BYTES + usize::from(slot & 0x1fff);
                let mask = record + model.records[target][row].len() - 1;
                bytes[mask] &= if model.tuple { !0b110 } else { !0b010 };
            }
        }
        if deleted.is_empty() && (target != 0 || nulled.is_empty()) {
            continue;
        }

        write_leaf(&mut bytes, model, target, deleted, nulled);
    }

    Expected {
        bytes,
        rows,
        removed: deleted.iter().map(|rows| rows.len()).sum(),
    }
}

fn write_leaf(bytes: &mut [u8], model: &Model, target: usize, deleted: &[usize], nulled: &[usize]) {
    // All fixtures have independent uncompressed leaf roots. Preserve the
    // inactive tail exactly while replacing live key/ROW entries and bitmap.
    let start = root(target) * PAGE_BYTES;
    bytes[start + INDEX_BITMAP..start + INDEX_START].fill(0);
    let physical = definition(target) * PAGE_BYTES + model.logical_start - PHYSICAL_BYTES;
    let ignored = model.bytes[physical + PHYSICAL_FLAGS] & 0x02 != 0;
    let mut entries = Vec::new();
    for (row, values) in model.rows[target].iter().enumerate() {
        if deleted.contains(&row) {
            continue;
        }
        let mut key = Vec::new();
        let columns: &[usize] = if model.tuple { &[2, 1] } else { &[1] };
        let null_assignment = target == 0 && nulled.contains(&row);
        if ignored
            && (null_assignment || columns.iter().all(|&column| values[column] == Value::Null))
        {
            continue;
        }
        for &column in columns {
            if null_assignment {
                key.push(0);
            } else {
                match values[column] {
                    Value::Null => key.push(0),
                    Value::Int32(value) => {
                        key.push(127);
                        key.extend(((value as u32) ^ 0x80000000).to_be_bytes());
                    }
                    _ => panic!("Literal model contains an unexpected carrier"),
                }
            }
        }
        key.extend((((data(target) as u32) << 8) | row as u32).to_be_bytes());
        entries.push(key);
    }
    entries.sort();
    let mut end = 0;
    for entry in entries {
        let at = start + INDEX_START + end;
        bytes[at..at + entry.len()].copy_from_slice(&entry);
        end += entry.len();
        bytes[start + INDEX_BITMAP + end / 8] |= 1 << (end % 8);
    }
    support::put16(bytes, start + 2, (PAGE_BYTES - INDEX_START - end) as u16);
}

pub fn verify(model: &Model, expected: &Expected, candidate: &Ace16DeletePreview) {
    assert_eq!(candidate.before_bytes(), model.bytes);
    assert_eq!(candidate.deleted_rows(), expected.removed);
    assert_eq!(candidate.candidate_bytes(), expected.bytes);
    qualify(expected);
}

pub fn qualify(expected: &Expected) {
    for target in 0..3 {
        let actual = read_table(
            &mut Cursor::new(&expected.bytes),
            CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(actual.rows(), expected.rows[target]);
        let checked = check_table(
            &mut Cursor::new(&expected.bytes),
            CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        check_table_constraints(
            &mut Cursor::new(&expected.bytes),
            CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
}
