//! Independent six-row, four-physical-index format goldens.
use super::{fixture, support};
use std::io::Cursor;
use tablecraft_ace::{
    Ace16DeletePreview, EditError, IndexCheckStatus, NativeImage, RowDelete, check_table,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

pub const PAGE_BYTES: usize = 4096;
pub const DEFINITION: usize = 10;
pub const DATA: usize = 12;
pub const OWNED_MAP: usize = 13;
pub const ROOTS: [usize; 4] = [14, 16, 18, 20];
pub const LEAVES: [usize; 2] = [21, 22];
pub const FREE: usize = 2;
pub const OWNER: usize = 4;
pub const PREVIOUS: usize = 12;
pub const NEXT: usize = 16;
pub const TAIL: usize = 20;
pub const PREFIX: usize = 24;
pub const HEIGHT: usize = 26;
pub const BITMAP: usize = 27;
pub const ENTRY_DATA: usize = 480;
pub const SLOT_DIRECTORY: usize = 14;
const COLUMN_START: usize = 63;
const COLUMN_BYTES: usize = 25;
const COUNTER_BYTES: usize = 12;
const PHYSICAL_BYTES: usize = 52;
const LOGICAL_BYTES: usize = 28;
const TABLE_ROWS: usize = 16;
const LOGICAL_COUNT: usize = 47;
const PHYSICAL_COUNT: usize = 51;
const KEY_START: usize = 4;
const KEY_BYTES: usize = 3;
const KEY_COUNT: usize = 10;
const USAGE: usize = 34;
const ROOT: usize = 38;
const FLAGS: usize = 46;
const LOGICAL_NUMBER: usize = 4;
const PHYSICAL_NUMBER: usize = 8;
pub const ROWS: [[Option<i32>; 2]; 6] = [
    [None, None],
    [None, Some(30)],
    [Some(10), None],
    [Some(20), Some(40)],
    [Some(20), None],
    [Some(30), Some(60)],
];

pub struct Model {
    pub bytes: Vec<u8>,
    pub records: Vec<Vec<u8>>,
    pub entries: [Vec<Vec<u8>>; 4],
    pub physical_start: usize,
}

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 30_000_000)
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

pub fn requests(model: &Model, rows: &[usize]) -> Vec<RowDelete> {
    rows.iter()
        .map(|&row| RowDelete::new(row, model.records[row].clone()))
        .collect()
}

pub fn candidate(model: &Model, rows: &[usize]) -> Result<Ace16DeletePreview, EditError> {
    capture(&model.bytes).preview_ace16_deletes(
        "Items",
        &requests(model, rows),
        &mut budget(),
        &|| false,
    )
}

pub fn slot(entry: &[u8]) -> usize {
    let reference = u32::from_be_bytes(entry[entry.len() - 4..].try_into().unwrap());
    assert_eq!(reference >> 8, DATA as u32);
    (reference & 255) as usize
}

fn key(values: &[Option<i32>], descending: &[bool], row: usize) -> Vec<u8> {
    let mut result = Vec::new();
    for (&value, &reverse) in values.iter().zip(descending) {
        result.push(match (value, reverse) {
            (None, false) => 0,
            (None, true) => 255,
            (Some(_), false) => 127,
            (Some(_), true) => 128,
        });
        if let Some(value) = value {
            for byte in ((value as u32) ^ 0x80000000).to_be_bytes() {
                result.push(if reverse { !byte } else { byte });
            }
        }
    }
    result.extend(((DATA as u32) << 8 | row as u32).to_be_bytes());
    result
}

pub fn page(height: u8, entries: &[Vec<u8>], previous: usize, next: usize, tail: usize) -> Vec<u8> {
    let mut result = vec![0xa7; PAGE_BYTES];
    result[..ENTRY_DATA].fill(0);
    result[..2].copy_from_slice(&[if height == 0 { 4 } else { 3 }, 1]);
    result[HEIGHT] = height;
    support::put32(&mut result, OWNER, DEFINITION as u32);
    support::put32(&mut result, PREVIOUS, previous as u32);
    support::put32(&mut result, NEXT, next as u32);
    support::put32(&mut result, TAIL, tail as u32);
    let mut length = 0;
    for entry in entries {
        result[ENTRY_DATA + length..ENTRY_DATA + length + entry.len()].copy_from_slice(entry);
        length += entry.len();
        result[BITMAP + length / 8] |= 1 << (length % 8);
    }
    support::put16(&mut result, FREE, (PAGE_BYTES - ENTRY_DATA - length) as u16);
    result
}

pub fn database(code: u8) -> Model {
    let fields = [("Id", 4, 0, 4), ("Score", 4, 4, 4), ("B", 4, 8, 4)];
    let mut definition = fixture::definition(&fields, ROWS.len() as u32, 11 << 8);
    let names_end = COLUMN_START
        + fields.len() * COLUMN_BYTES
        + fields
            .iter()
            .map(|field| 2 + field.0.encode_utf16().count() * 2)
            .sum::<usize>();
    let counters = COUNTER_BYTES * ROOTS.len();
    definition.copy_within(COLUMN_START..names_end, COLUMN_START + counters);
    definition[COLUMN_START..COLUMN_START + counters].fill(0);
    support::put32(&mut definition, LOGICAL_COUNT, 5);
    support::put32(&mut definition, PHYSICAL_COUNT, 4);
    let physical_start = names_end + counters;
    let keys: [&[(usize, bool)]; 4] = [
        &[(1, false)],
        &[(1, false)],
        &[(2, true)],
        &[(1, false), (2, true)],
    ];
    let entries = std::array::from_fn(|index| {
        let mut entries = Vec::new();
        for (row, values) in ROWS.iter().enumerate() {
            let selected = keys[index]
                .iter()
                .map(|&(column, _)| values[column - 1])
                .collect::<Vec<_>>();
            if index != 0 && selected.iter().all(Option::is_none) {
                continue;
            }
            let directions = keys[index]
                .iter()
                .map(|&(_, descending)| descending)
                .collect::<Vec<_>>();
            entries.push(key(&selected, &directions, row));
        }
        entries.sort();
        entries
    });
    for index in 0..ROOTS.len() {
        let at = physical_start + PHYSICAL_BYTES * index;
        for component in 0..KEY_COUNT {
            let offset = at + KEY_START + component * KEY_BYTES;
            support::put16(
                &mut definition,
                offset,
                keys[index]
                    .get(component)
                    .map_or(0xffff, |key| key.0 as u16),
            );
            if let Some(&(_, descending)) = keys[index].get(component) {
                definition[offset + 2] = u8::from(!descending);
            }
        }
        support::put32(
            &mut definition,
            at + USAGE,
            ((OWNED_MAP as u32) << 8) | index as u32,
        );
        support::put32(&mut definition, at + ROOT, ROOTS[index] as u32);
        definition[at + FLAGS] = [0x80, 0x82, 0x83, 0x82][index];
        support::put32(
            &mut definition,
            COLUMN_START + COUNTER_BYTES * index + 4,
            entries[index].len() as u32,
        );
    }
    let logical_start = physical_start + PHYSICAL_BYTES * ROOTS.len();
    let mut name_offset = logical_start + LOGICAL_BYTES * 5;
    for (logical, (name, physical)) in [
        ("Ordinary", 0),
        ("IgnoredA", 1),
        ("AliasA", 1),
        ("UniqueB", 2),
        ("Mixed", 3),
    ]
    .into_iter()
    .enumerate()
    {
        let at = logical_start + logical * LOGICAL_BYTES;
        support::put32(&mut definition, at + LOGICAL_NUMBER, logical as u32);
        support::put32(&mut definition, at + PHYSICAL_NUMBER, physical);
        let label = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(&mut definition, name_offset, label.len() as u16);
        name_offset += 2;
        definition[name_offset..name_offset + label.len()].copy_from_slice(&label);
        name_offset += label.len();
    }
    let mut bytes = fixture::with_rule("Caption", "independent ignored deletion");
    bytes.resize(23 * PAGE_BYTES, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes[DEFINITION * PAGE_BYTES..(DEFINITION + 1) * PAGE_BYTES].copy_from_slice(&definition);
    let records = ROWS
        .iter()
        .enumerate()
        .map(|(row, values)| {
            let mut record = 3_u16.to_le_bytes().to_vec();
            record.extend((row as i32 + 1).to_le_bytes());
            let mut mask = 1;
            for (column, value) in values.iter().enumerate() {
                record.extend(value.unwrap_or(0).to_le_bytes());
                if value.is_some() {
                    mask |= 1 << (column + 1);
                }
            }
            record.push(mask);
            record
        })
        .collect::<Vec<_>>();
    let data_rows = records
        .iter()
        .cloned()
        .map(|record| (0, record))
        .collect::<Vec<_>>();
    bytes[DATA * PAGE_BYTES..(DATA + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(DEFINITION as u32, &data_rows));
    support::put32(&mut bytes, 11 * PAGE_BYTES + OWNER, 0);
    let mut owned_rows = Vec::new();
    for (index, &root) in ROOTS.iter().enumerate() {
        let mut usage = vec![0; 9];
        usage[5 + root / 8] |= 1 << (root % 8);
        if index == 1 {
            for leaf in LEAVES {
                usage[5 + leaf / 8] |= 1 << (leaf % 8);
            }
        }
        owned_rows.push((0, usage));
    }
    bytes[OWNED_MAP * PAGE_BYTES..(OWNED_MAP + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(0, &owned_rows));
    let mut global = vec![0; 69];
    for number in 23..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&support::data_page(1, &[(0, global)]));
    for (index, &root) in ROOTS.iter().enumerate() {
        if index != 1 {
            bytes[root * PAGE_BYTES..(root + 1) * PAGE_BYTES].copy_from_slice(&page(
                0,
                &entries[index],
                0,
                0,
                0,
            ));
        }
    }
    for (position, leaf) in LEAVES.into_iter().enumerate() {
        let start = position * 2;
        let previous = if position == 0 { 0 } else { LEAVES[0] };
        let next = if position == 0 { LEAVES[1] } else { 0 };
        bytes[leaf * PAGE_BYTES..(leaf + 1) * PAGE_BYTES].copy_from_slice(&page(
            0,
            &entries[1][start..start + 2],
            previous,
            next,
            0,
        ));
    }
    let mut bound = entries[1][1].clone();
    bound.extend((LEAVES[0] as u32).to_be_bytes());
    bytes[ROOTS[1] * PAGE_BYTES..(ROOTS[1] + 1) * PAGE_BYTES].copy_from_slice(&page(
        1,
        &[bound],
        0,
        0,
        LEAVES[1],
    ));
    Model {
        bytes,
        records,
        entries,
        physical_start,
    }
}

pub fn decoded_entries(bytes: &[u8], number: usize) -> Vec<Vec<u8>> {
    let page = &bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES];
    let prefix = u16::from_le_bytes(page[PREFIX..PREFIX + 2].try_into().unwrap()) as usize;
    let mut result: Vec<Vec<u8>> = Vec::new();
    let mut start = 0;
    for end in 1..PAGE_BYTES - ENTRY_DATA {
        if page[BITMAP + end / 8] & (1 << (end % 8)) == 0 {
            continue;
        }
        let mut entry = if result.is_empty() {
            Vec::new()
        } else {
            result[0][..prefix].to_vec()
        };
        entry.extend(&page[ENTRY_DATA + start..ENTRY_DATA + end]);
        result.push(entry);
        start = end;
    }
    result
}

pub fn checked(bytes: &[u8], count: usize) {
    let report = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), count);
    assert_eq!(report.schema().declared_rows() as usize, count);
    assert_eq!(report.indexes().len(), 5);
    for index in report.indexes() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
    }
}

pub fn verify(model: &Model, output: &Ace16DeletePreview, deleted: &[usize]) {
    let bytes = output.candidate_bytes();
    assert_eq!(output.before_bytes(), model.bytes);
    assert_eq!(output.deleted_rows(), deleted.len());
    checked(bytes, ROWS.len() - deleted.len());
    let count_offset = DEFINITION * PAGE_BYTES + TABLE_ROWS;
    assert_eq!(
        &bytes[count_offset..count_offset + size_of::<u32>()],
        &((ROWS.len() - deleted.len()) as u32).to_le_bytes(),
    );
    for index in 0..ROOTS.len() {
        let expected = model.entries[index]
            .iter()
            .filter(|entry| !deleted.contains(&slot(entry)))
            .cloned()
            .collect::<Vec<_>>();
        let actual = if index == 1 && bytes[ROOTS[index] * PAGE_BYTES + HEIGHT] != 0 {
            let mut actual = Vec::new();
            for leaf in LEAVES {
                if bytes[leaf * PAGE_BYTES] != 0 {
                    actual.extend(decoded_entries(bytes, leaf));
                }
            }
            actual
        } else {
            decoded_entries(bytes, ROOTS[index])
        };
        assert_eq!(actual, expected, "physical index {index}");
        if model.entries[index]
            .iter()
            .all(|entry| !deleted.contains(&slot(entry)))
        {
            for number in std::iter::once(ROOTS[index]).chain(if index == 1 {
                LEAVES.to_vec()
            } else {
                Vec::new()
            }) {
                assert_eq!(
                    &bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES],
                    &model.bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES]
                );
            }
        }
    }
    for row in 0..ROWS.len() {
        let offset = DATA * PAGE_BYTES + SLOT_DIRECTORY + row * 2;
        let before = u16::from_le_bytes(model.bytes[offset..offset + 2].try_into().unwrap());
        let expected = if deleted.contains(&row) {
            before | 0xc000
        } else {
            before
        };
        assert_eq!(&bytes[offset..offset + 2], &expected.to_le_bytes());
        let start = (before & 0x1fff) as usize + DATA * PAGE_BYTES;
        assert_eq!(
            &bytes[start..start + model.records[row].len()],
            model.records[row]
        );
    }
    // Insertion/history counters and the entire logical/physical declaration stay exact.
    let offset = DEFINITION * PAGE_BYTES;
    assert_eq!(
        &bytes[offset + COLUMN_START..offset + PAGE_BYTES],
        &model.bytes[offset + COLUMN_START..offset + PAGE_BYTES]
    );
}
