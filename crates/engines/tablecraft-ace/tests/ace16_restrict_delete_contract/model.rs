//! Repository-authored reciprocal cross-table keys; no native encoder is used.
use super::{fixture, support};
use std::io::Cursor;
use tablecraft_ace::{
    Ace16DeletePreview, EditError, IndexCheckStatus, NativeImage, RowDelete, check_table,
    check_table_constraints, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

pub const PAGE_BYTES: usize = 4096;
pub const LOGICAL_BYTES: usize = 28;
pub const FOREIGN_ROLE: usize = 12;
pub const FOREIGN_INDEX: usize = 13;
pub const FOREIGN_TABLE: usize = 17;
pub const UPDATE_ACTION: usize = 21;
pub const DELETE_ACTION: usize = 22;
pub const INDEX_KIND: usize = 23;
const COLUMN_START: usize = 63;
const COLUMN_BYTES: usize = 25;
const COUNTER_BYTES: usize = 12;
const PHYSICAL_BYTES: usize = 52;
const KEY_START: usize = 4;
const KEY_BYTES: usize = 3;
const KEY_COUNT: usize = 10;
const USAGE: usize = 34;
const ROOT: usize = 38;
const FLAGS: usize = 46;
const ENTRY_START: usize = 480;
const BITMAP: usize = 27;
const OWNER: usize = 4;
const DATA_SLOT_START: usize = 14;

pub struct Model {
    pub bytes: Vec<u8>,
    pub records: [Vec<Vec<u8>>; 3],
    pub rows: [Vec<Vec<Value>>; 3],
    pub logical_start: usize,
    pub tuple: bool,
    pub second: bool,
}

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 40_000_000)
}

pub fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

pub fn table(index: usize) -> &'static str {
    ["Items", "Parents", "Other"][index]
}

pub fn definition(index: usize) -> usize {
    10 + index * 5
}
pub fn data(index: usize) -> usize {
    definition(index) + 2
}
pub fn root(index: usize) -> usize {
    definition(index) + 4
}

pub fn requests(model: &Model, target: usize, deleted: &[usize]) -> Vec<RowDelete> {
    deleted
        .iter()
        .map(|&row| RowDelete::new(row, model.records[target][row].clone()))
        .collect()
}

pub fn candidate(
    model: &Model,
    target: usize,
    deleted: &[usize],
) -> Result<Ace16DeletePreview, EditError> {
    capture(&model.bytes).preview_ace16_deletes(
        table(target),
        &requests(model, target, deleted),
        &mut budget(),
        &|| false,
    )
}

pub fn relationship(name: &str, flags: u32, child: &str, tuple: bool) -> Vec<Vec<u8>> {
    let columns: &[&str] = if tuple { &["Score", "K2"] } else { &["Score"] };
    columns
        .iter()
        .enumerate()
        .map(|(ordinal, column)| {
            let mut row = 8_u16.to_le_bytes().to_vec();
            for value in [flags, columns.len() as u32, ordinal as u32] {
                row.extend(value.to_le_bytes());
            }
            let mut offsets = vec![row.len() as u16];
            for text in [name, "Parents", child, column, column] {
                row.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
                offsets.push(row.len() as u16);
            }
            for offset in offsets.into_iter().rev() {
                row.extend(offset.to_le_bytes());
            }
            row.extend(5_u16.to_le_bytes());
            row.push(0xff);
            row
        })
        .collect()
}

pub fn relationships(model: &mut Model, flags: u32) {
    let mut records = relationship("First", flags, "Items", model.tuple);
    if model.second {
        records.extend(relationship("Second", flags, "Other", model.tuple));
    }
    support::put32(&mut model.bytes, 7 * PAGE_BYTES + 16, records.len() as u32);
    let rows = records.into_iter().map(|row| (0, row)).collect::<Vec<_>>();
    model.bytes[9 * PAGE_BYTES..10 * PAGE_BYTES].copy_from_slice(&support::data_page(7, &rows));
}

pub fn database(code: u8, tuple: bool, second: bool, update_cascade: bool) -> Model {
    let values = [
        if tuple {
            vec![
                [Some(10), Some(100)],
                [Some(20), None],
                [None, None],
                [Some(10), Some(100)],
            ]
        } else {
            vec![
                [Some(10), Some(100)],
                [Some(10), Some(200)],
                [None, None],
                [None, Some(400)],
            ]
        },
        vec![
            [Some(10), Some(100)],
            [Some(20), None],
            [Some(30), Some(300)],
        ],
        vec![[Some(20), None]],
    ];
    let rows = std::array::from_fn(|index| {
        values[index]
            .iter()
            .enumerate()
            .map(|(ordinal, pair)| {
                vec![
                    Value::Int32(ordinal as i32 + 1),
                    pair[0].map_or(Value::Null, Value::Int32),
                    pair[1].map_or(Value::Null, Value::Int32),
                ]
            })
            .collect()
    });
    let records = std::array::from_fn(|index| {
        values[index]
            .iter()
            .enumerate()
            .map(|(ordinal, pair)| {
                let mut record = 3_u16.to_le_bytes().to_vec();
                record.extend((ordinal as i32 + 1).to_le_bytes());
                let mut mask = 1;
                for (column, value) in pair.iter().enumerate() {
                    record.extend(value.unwrap_or(0).to_le_bytes());
                    if value.is_some() {
                        mask |= 1 << (column + 1);
                    }
                }
                record.push(mask);
                record
            })
            .collect::<Vec<_>>()
    });
    let fields = [("Id", 4, 0, 4), ("Score", 4, 4, 4), ("K2", 4, 8, 4)];
    let names_end = COLUMN_START
        + fields.len() * COLUMN_BYTES
        + fields
            .iter()
            .map(|field| 2 + field.0.len() * 2)
            .sum::<usize>();
    let physical = names_end + COUNTER_BYTES;
    let logical_start = physical + PHYSICAL_BYTES;
    let mut bytes = fixture::database();
    bytes.resize(25 * PAGE_BYTES, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    for index in 0..3 {
        let page_number = definition(index);
        let logical_count = if index == 1 {
            2 + usize::from(second)
        } else {
            2
        };
        let mut page = fixture::definition(
            &fields,
            values[index].len() as u32,
            ((page_number + 1) << 8) as u32,
        );
        page.copy_within(COLUMN_START..names_end, COLUMN_START + COUNTER_BYTES);
        page[COLUMN_START..COLUMN_START + COUNTER_BYTES].fill(0);
        support::put32(&mut page, COLUMN_START + 4, values[index].len() as u32);
        support::put32(&mut page, 47, logical_count as u32);
        support::put32(&mut page, 51, 1);
        for component in 0..KEY_COUNT {
            let at = physical + KEY_START + component * KEY_BYTES;
            support::put16(
                &mut page,
                at,
                if component < if tuple { 2 } else { 1 } {
                    component as u16 + 1
                } else {
                    0xffff
                },
            );
            page[at + 2] = 1;
        }
        support::put32(&mut page, physical + USAGE, ((page_number + 3) << 8) as u32);
        support::put32(&mut page, physical + ROOT, root(index) as u32);
        page[physical + FLAGS] = if index == 1 { 0x81 } else { 0x80 };
        let mut names = logical_start + logical_count * LOGICAL_BYTES;
        for logical in 0..logical_count {
            let at = logical_start + logical * LOGICAL_BYTES;
            support::put32(&mut page, at + 4, logical as u32);
            let foreign = if index == 1 {
                logical != 0
            } else {
                logical == 0 && (index == 0 || second)
            };
            if foreign {
                let other = if index == 1 {
                    if logical == 1 { 0 } else { 2 }
                } else {
                    1
                };
                page[at + FOREIGN_ROLE] = if index == 1 { 1 } else { 2 };
                support::put32(
                    &mut page,
                    at + FOREIGN_INDEX,
                    if index == 1 {
                        0
                    } else {
                        if index == 0 { 1 } else { 2 }
                    },
                );
                support::put32(&mut page, at + FOREIGN_TABLE, definition(other) as u32);
                page[at + UPDATE_ACTION] = u8::from(update_cascade);
                page[at + INDEX_KIND] = 2;
            }
            let label = if foreign {
                format!("Link{logical}")
            } else {
                format!("Alias{logical}")
            };
            let encoded = label
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            support::put16(&mut page, names, encoded.len() as u16);
            names += 2;
            page[names..names + encoded.len()].copy_from_slice(&encoded);
            names += encoded.len();
        }
        bytes[page_number * PAGE_BYTES..(page_number + 1) * PAGE_BYTES].copy_from_slice(&page);
        let records = records[index]
            .iter()
            .cloned()
            .map(|row| (0, row))
            .collect::<Vec<_>>();
        bytes[data(index) * PAGE_BYTES..(data(index) + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(page_number as u32, &records));
        let mut map = vec![0; 9];
        map[5 + data(index) / 8] |= 1 << (data(index) % 8);
        bytes[(page_number + 1) * PAGE_BYTES..(page_number + 2) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(0, &[(0, map)]));
        let mut map = vec![0; 9];
        map[5 + root(index) / 8] |= 1 << (root(index) % 8);
        bytes[(page_number + 3) * PAGE_BYTES..(page_number + 4) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(0, &[(0, map)]));
        let mut entries = Vec::new();
        for (ordinal, pair) in values[index].iter().enumerate() {
            let mut entry = Vec::new();
            for value in pair.iter().take(if tuple { 2 } else { 1 }) {
                entry.push(if value.is_some() { 127 } else { 0 });
                if let Some(value) = value {
                    entry.extend(((*value as u32) ^ 0x80000000).to_be_bytes());
                }
            }
            entry.extend((((data(index) as u32) << 8) | ordinal as u32).to_be_bytes());
            entries.push(entry);
        }
        entries.sort();
        let mut page = vec![0; PAGE_BYTES];
        page[..2].copy_from_slice(&[4, 1]);
        support::put32(&mut page, OWNER, page_number as u32);
        let mut end = 0;
        for entry in entries {
            page[ENTRY_START + end..ENTRY_START + end + entry.len()].copy_from_slice(&entry);
            end += entry.len();
            page[BITMAP + end / 8] |= 1 << (end % 8);
        }
        support::put16(&mut page, 2, (PAGE_BYTES - ENTRY_START - end) as u16);
        bytes[root(index) * PAGE_BYTES..(root(index) + 1) * PAGE_BYTES].copy_from_slice(&page);
    }
    let mut catalog = vec![
        (
            0,
            support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        ),
        (
            0,
            support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        ),
    ];
    for index in 0..3 {
        let mut name = vec![255, 254];
        name.extend(table(index).as_bytes());
        catalog.push((0, support::record(definition(index) as i32, 0, 1, 0, &name)));
    }
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&support::data_page(2, &catalog));
    support::put32(&mut bytes, 2 * PAGE_BYTES + 16, catalog.len() as u32);
    bytes[8 * PAGE_BYTES..9 * PAGE_BYTES]
        .copy_from_slice(&support::data_page(0, &[(0, vec![0, 0, 0, 0, 0, 0, 2])]));
    let mut global = vec![0; 69];
    for page in 25..512 {
        global[5 + page / 8] |= 1 << (page % 8);
    }
    bytes[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&support::data_page(1, &[(0, global)]));
    let mut model = Model {
        bytes,
        records,
        rows,
        logical_start,
        tuple,
        second,
    };
    relationships(&mut model, if update_cascade { 0x100 } else { 0 });
    model
}

pub fn verify(model: &Model, output: &Ace16DeletePreview, target: usize, deleted: &[usize]) {
    assert_eq!(output.before_bytes(), model.bytes);
    assert_eq!(output.deleted_rows(), deleted.len());
    for index in 0..3 {
        let expected = model.rows[index]
            .iter()
            .enumerate()
            .filter(|(row, _)| index != target || !deleted.contains(row))
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>();
        let actual = read_table(
            &mut Cursor::new(output.candidate_bytes()),
            CatalogScope::new(1),
            table(index),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(actual.rows(), expected);
        let report = check_table(
            &mut Cursor::new(output.candidate_bytes()),
            CatalogScope::new(1),
            table(index),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            report
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        check_table_constraints(
            &mut Cursor::new(output.candidate_bytes()),
            CatalogScope::new(1),
            table(index),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
    let after = output.candidate_bytes();
    for index in 0..3 {
        for row in 0..model.records[index].len() {
            let offset = data(index) * PAGE_BYTES + DATA_SLOT_START + row * 2;
            let word = u16::from_le_bytes(model.bytes[offset..offset + 2].try_into().unwrap());
            let expected = if index == target && deleted.contains(&row) {
                word | 0xc000
            } else {
                word
            };
            assert_eq!(&after[offset..offset + 2], &expected.to_le_bytes());
            let start = data(index) * PAGE_BYTES + (word & 0x1fff) as usize;
            assert_eq!(
                &after[start..start + model.records[index][row].len()],
                model.records[index][row]
            );
        }
        if index != target {
            let start = definition(index) * PAGE_BYTES;
            assert_eq!(
                &after[start..start + 5 * PAGE_BYTES],
                &model.bytes[start..start + 5 * PAGE_BYTES]
            );
        }
    }
}
