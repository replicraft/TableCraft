use super::{cascade_model, restrict_model, support};
use tablecraft_core::values::Value;

const PAGE_BYTES: usize = 4096;
const PHYSICAL_BYTES: usize = 52;
const KEY_COLUMNS_OFFSET: usize = 4;
const KEY_COMPONENT_BYTES: usize = 3;
const TABLE_ROWS: usize = 16;
const INDEX_ENTRIES: usize = 480;
const INDEX_MASK: usize = 27;

pub fn database() -> restrict_model::Model {
    let mut model = restrict_model::database(6, true, true, false);
    cascade_model::primary(&mut model, 1);
    model.rows[0][1][2] = Value::Int32(200);
    model.rows[1][1][2] = Value::Int32(200);
    model.rows[2][0][2] = Value::Int32(200);
    for target in 0..3 {
        model.records[target] = model.rows[target]
            .iter()
            .map(|values| {
                let mut row = 3_u16.to_le_bytes().to_vec();
                let mut mask = 0;
                for (column, value) in values.iter().enumerate() {
                    match value {
                        Value::Int32(value) => {
                            row.extend(value.to_le_bytes());
                            mask |= 1 << column;
                        }
                        Value::Null => row.extend(0_i32.to_le_bytes()),
                        _ => panic!("Independent tuple fixture contains a nonintegral value"),
                    }
                }
                row.push(mask);
                row
            })
            .collect();
        let data = cascade_model::data(target);
        let records: Vec<_> = model.records[target]
            .iter()
            .cloned()
            .map(|row| (0, row))
            .collect();
        model.bytes[data * PAGE_BYTES..(data + 1) * PAGE_BYTES].copy_from_slice(
            &support::data_page(cascade_model::definition(target) as u32, &records),
        );
        let physical =
            cascade_model::definition(target) * PAGE_BYTES + model.logical_start - PHYSICAL_BYTES;
        support::put16(&mut model.bytes, physical + KEY_COLUMNS_OFFSET, 2);
        support::put16(
            &mut model.bytes,
            physical + KEY_COLUMNS_OFFSET + KEY_COMPONENT_BYTES,
            1,
        );
        let mut entries = Vec::new();
        for (ordinal, row) in model.rows[target].iter().enumerate() {
            let mut encoded = Vec::new();
            for column in [2, 1] {
                match row[column] {
                    Value::Null => encoded.push(0),
                    Value::Int32(value) => {
                        encoded.push(127);
                        encoded.extend(((value as u32) ^ 0x80000000).to_be_bytes());
                    }
                    _ => unreachable!(),
                }
            }
            encoded.extend((((data as u32) << 8) | ordinal as u32).to_be_bytes());
            entries.push(encoded);
        }
        entries.sort();
        let mut root = vec![0; PAGE_BYTES];
        root[..2].copy_from_slice(&[4, 1]);
        support::put32(&mut root, 4, cascade_model::definition(target) as u32);
        let mut end = 0;
        for entry in entries {
            root[INDEX_ENTRIES + end..INDEX_ENTRIES + end + entry.len()].copy_from_slice(&entry);
            end += entry.len();
            root[INDEX_MASK + end / 8] |= 1 << (end % 8);
        }
        support::put16(&mut root, 2, (PAGE_BYTES - INDEX_ENTRIES - end) as u16);
        let page = cascade_model::root(target);
        model.bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES].copy_from_slice(&root);
    }
    let mut relationships = Vec::new();
    for (name, child) in [("First", "Items"), ("Second", "Other")] {
        for (ordinal, column) in ["K2", "Score"].into_iter().enumerate() {
            let mut row = 8_u16.to_le_bytes().to_vec();
            for value in [0x1000, 2, ordinal as u32] {
                row.extend(value.to_le_bytes());
            }
            let mut offsets = vec![row.len() as u16];
            for value in [name, "Parents", child, column, column] {
                row.extend(value.encode_utf16().flat_map(u16::to_le_bytes));
                offsets.push(row.len() as u16);
            }
            for offset in offsets.into_iter().rev() {
                row.extend(offset.to_le_bytes());
            }
            row.extend(5_u16.to_le_bytes());
            row.push(0xff);
            relationships.push((0, row));
        }
    }
    support::put32(&mut model.bytes, 7 * PAGE_BYTES + TABLE_ROWS, 4);
    model.bytes[9 * PAGE_BYTES..10 * PAGE_BYTES]
        .copy_from_slice(&support::data_page(7, &relationships));
    for (target, logical) in [(0, 0), (1, 1), (1, 2), (2, 0)] {
        let at =
            cascade_model::definition(target) * PAGE_BYTES + model.logical_start + logical * 28;
        model.bytes[at + 22] = 1;
    }
    model
}
