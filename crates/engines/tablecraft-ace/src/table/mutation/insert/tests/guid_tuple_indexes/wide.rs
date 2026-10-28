//! Ten complete GUID components reach the real 198-byte internal-entry limit.
use super::super::{model as original, multiple_indexes::model as frames};
use crate::{EditError, IndexConstraintViolation, InspectError, RowInsert, test_support::base};
use std::io::Cursor;
use tablecraft_core::values::{Guid, Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as raw;

const PAGE: usize = 4096;
const GUIDS: usize = 10;
const RECORD_BYTES: usize = 168;

#[derive(Clone)]
struct Row {
    id: i32,
    members: [Option<[u8; 16]>; GUIDS],
}

impl Row {
    fn values(&self) -> Vec<Value> {
        let mut values = vec![Value::Int32(self.id)];
        values.extend(self.members.iter().map(|member| {
            member.map_or(Value::Null, |bytes| Value::Guid(Guid::from_bytes(bytes)))
        }));
        values
    }
}

struct Case {
    before: Vec<u8>,
    rows: Vec<(Row, u32)>,
    columns: Vec<(usize, bool)>,
    ignored: bool,
}

fn guid(seed: u8, component: usize) -> [u8; 16] {
    [
        seed,
        1,
        2,
        3,
        4,
        5,
        component as u8,
        7,
        0x80,
        9,
        10,
        11,
        12,
        13,
        14,
        0xff,
    ]
}

fn record(row: &Row, null_fill: u8) -> Vec<u8> {
    let mut bytes = 11_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    let mut mask = 1_u16;
    for (position, member) in row.members.iter().enumerate() {
        let mut stored = member.unwrap_or([null_fill; 16]);
        stored[..4].reverse();
        stored[4..6].reverse();
        stored[6..8].reverse();
        bytes.extend(stored);
        if member.is_some() {
            mask |= 1 << (position + 1);
        }
    }
    bytes.extend(mask.to_le_bytes());
    assert_eq!(bytes.len(), RECORD_BYTES);
    bytes
}

fn component(value: Option<[u8; 16]>, descending: bool) -> Vec<u8> {
    let Some(value) = value else {
        return vec![if descending { 255 } else { 0 }];
    };
    let mut bytes = vec![if descending { 128 } else { 127 }];
    bytes.extend(
        value[..8]
            .iter()
            .map(|byte| if descending { !byte } else { *byte }),
    );
    bytes.push(9);
    bytes.extend(
        value[8..]
            .iter()
            .map(|byte| if descending { !byte } else { *byte }),
    );
    bytes.push(if descending { 247 } else { 8 });
    bytes
}

fn entries(rows: &[(Row, u32)], case: &Case, secondary: bool) -> Vec<Vec<u8>> {
    let mut entries = Vec::new();
    for (row, reference) in rows {
        let mut wire = Vec::new();
        if secondary {
            if case.ignored && row.members.iter().all(Option::is_none) {
                continue;
            }
            for (column, descending) in &case.columns {
                wire.extend(component(row.members[*column - 1], *descending));
            }
        } else {
            wire.push(127);
            wire.extend(((row.id as u32) ^ 0x80000000).to_be_bytes());
        }
        wire.extend(reference.to_be_bytes());
        entries.push(wire);
    }
    entries.sort();
    entries
}

fn source(code: u8, reversed: bool, ignored: bool, pressure: bool) -> Case {
    let columns = (0..GUIDS)
        .map(|position| {
            let column = if reversed {
                GUIDS - position
            } else {
                position + 1
            };
            (column, (position % 2 != 0) ^ reversed)
        })
        .collect::<Vec<_>>();
    let vary = columns.last().unwrap().0 - 1;
    let mut records = if pressure {
        (0..18)
            .map(|position| Row {
                id: position + 1,
                members: std::array::from_fn(|component| Some(guid(position as u8, component))),
            })
            .collect::<Vec<_>>()
    } else {
        let first = Row {
            id: 1,
            members: std::array::from_fn(|component| Some(guid(5, component))),
        };
        let mut second = first.clone();
        second.id = 2;
        second.members[vary] = Some(guid(6, vary));
        vec![first, second]
    };
    records.push(Row {
        id: records.len() as i32 + 1,
        members: [None; GUIDS],
    });
    let fields = [
        "G0", "G1", "G2", "G3", "G4", "G5", "G6", "G7", "G8", "Score",
    ];
    let mut metadata = vec![("ID", 4, 0, 4)];
    for (position, name) in fields.iter().enumerate() {
        metadata.push((*name, 15, (4 + position * 16) as u16, 16));
    }
    let mut definition = base::definition(&metadata, records.len() as u32, 11 << 8);
    let end = 63
        + 25 * metadata.len()
        + metadata
            .iter()
            .map(|(name, _, _, _)| 2 + name.encode_utf16().count() * 2)
            .sum::<usize>();
    definition.copy_within(63..end, 87);
    definition[63..87].fill(0);
    raw::put32(&mut definition, 67, 37);
    raw::put32(&mut definition, 79, 59);
    raw::put32(&mut definition, 47, 4);
    raw::put32(&mut definition, 51, 2);
    raw::put32(&mut definition, 59, (11 << 8) | 1);
    let physical = end + 24;
    let logical = physical + 2 * 52;
    let mut names = logical + 4 * 28;
    definition[physical..names].fill(0);
    for index in 0..2 {
        let at = physical + index * 52;
        for position in 0..10 {
            raw::put16(&mut definition, at + 4 + position * 3, 0xffff);
        }
        let selected = if index == 0 {
            vec![(0, false)]
        } else {
            columns.clone()
        };
        for (position, (number, descending)) in selected.iter().enumerate() {
            raw::put16(&mut definition, at + 4 + position * 3, *number as u16);
            definition[at + 6 + position * 3] = u8::from(!descending);
        }
        raw::put32(&mut definition, at + 34, (15 << 8) | index as u32);
        raw::put32(&mut definition, at + 38, 13 + index as u32);
        definition[at + 46] = if index == 0 {
            0x89
        } else {
            0x80 | u8::from(!pressure) | (u8::from(ignored) << 1)
        };
    }
    for (position, name) in ["PrimaryID", "PrimaryAlias", "WideOrder", "WideAlias"]
        .iter()
        .enumerate()
    {
        let at = logical + position * 28;
        raw::put32(&mut definition, at + 4, position as u32);
        raw::put32(&mut definition, at + 8, (position / 2) as u32);
        definition[at + 23] = u8::from(position == 0);
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        raw::put16(&mut definition, names, encoded.len() as u16);
        names += 2;
        definition[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    raw::put16(&mut definition, names, 0xffff);
    let mut before = frames::source(code, false, false).before;
    frames::install(&mut before, 10, &definition);
    let encoded = records
        .iter()
        .map(|row| (0, record(row, 0x5e)))
        .collect::<Vec<_>>();
    let mut page = raw::data_page(10, &encoded);
    raw::put16(
        &mut page,
        2,
        (PAGE - 14 - records.len() * (RECORD_BYTES + 2)) as u16,
    );
    frames::install(&mut before, 12, &page);
    let rows = records
        .into_iter()
        .enumerate()
        .map(|(slot, row)| (row, (12 << 8) | slot as u32))
        .collect();
    let mut case = Case {
        before,
        rows,
        columns,
        ignored,
    };
    for index in 0..2 {
        let wires = entries(&case.rows, &case, index == 1);
        let mut page = frames::leaf(None, &wires, 0, 0, 0);
        let active = wires.iter().map(Vec::len).sum::<usize>();
        page[480 + active..].fill(if index == 0 { 0xb6 } else { 0xc7 });
        frames::install(&mut case.before, 13 + index, &page);
    }
    case
}

fn expected(case: &Case, inserted: &Row) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let count = case.rows.len();
    let end = u16::from_le_bytes(
        bytes[12 * PAGE + 12 + count * 2..12 * PAGE + 14 + count * 2]
            .try_into()
            .unwrap(),
    ) as usize;
    let at = end - RECORD_BYTES;
    bytes[12 * PAGE + at..12 * PAGE + end].copy_from_slice(&record(inserted, 0));
    raw::put16(&mut bytes, 12 * PAGE + 14 + count * 2, at as u16);
    raw::put16(&mut bytes, 12 * PAGE + 12, (count + 1) as u16);
    raw::put16(
        &mut bytes,
        12 * PAGE + 2,
        (at - 14 - 2 * (count + 1)) as u16,
    );
    let mut rows = case.rows.clone();
    rows.push((inserted.clone(), (12 << 8) | count as u32));
    let mut secondary = vec![14];
    for index in 0..2 {
        let wires = entries(&rows, case, index == 1);
        let old = &case.before[(13 + index) * PAGE..(14 + index) * PAGE];
        if frames::framing(&wires, 0).1 <= 3616 {
            frames::install(
                &mut bytes,
                13 + index as u32,
                &frames::leaf(Some(old), &wires, 0, 0, 0),
            );
        } else {
            assert_eq!(index, 1);
            let right = (bytes.len() / PAGE) as u32;
            let left = right + 1;
            bytes.resize(bytes.len() + 2 * PAGE, 0);
            let middle = wires.len() / 2;
            frames::install(
                &mut bytes,
                right,
                &frames::leaf(None, &wires[middle..], 0, left, 0),
            );
            frames::install(
                &mut bytes,
                left,
                &frames::leaf(None, &wires[..middle], 0, 0, right),
            );
            frames::install(
                &mut bytes,
                14,
                &frames::root(old, left, right, &wires[middle - 1]),
            );
            secondary.extend([right, left]);
        }
    }
    raw::put32(&mut bytes, 10 * PAGE + 16, (count + 1) as u32);
    raw::put32(&mut bytes, 10 * PAGE + 67, 38);
    raw::put32(&mut bytes, 10 * PAGE + 79, 60);
    frames::map_page(
        &mut bytes,
        15,
        vec![frames::map([13]), frames::map(secondary)],
    );
    let eof = (bytes.len() / PAGE) as u32;
    frames::map_page(&mut bytes, 1, vec![frames::map(eof..512)]);
    bytes
}

fn typed(bytes: &[u8], rows: &[(Row, u32)]) {
    let table = crate::read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), rows.len());
    for (actual, (row, _)) in table.rows().iter().zip(rows) {
        assert_eq!(actual, &row.values());
    }
}

fn run(case: &Case, inserted: &Row) -> Vec<u8> {
    original::checked(&case.before, case.rows.len(), 4);
    typed(&case.before, &case.rows);
    let after = expected(case, inserted);
    original::checked(&after, case.rows.len() + 1, 4);
    let mut rows = case.rows.clone();
    rows.push((inserted.clone(), 0));
    typed(&after, &rows);
    let candidate = original::capture(&case.before)
        .preview_ace16_insert_rows(
            "Items",
            &[RowInsert::new(inserted.values())],
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
    let actual = candidate.candidate_bytes();
    assert_eq!(actual.len(), after.len());
    let difference = actual
        .iter()
        .zip(&after)
        .position(|(left, right)| left != right);
    assert!(
        difference.is_none(),
        "first wide complete-image difference at {difference:?}"
    );
    original::checked(actual, rows.len(), 4);
    typed(actual, &rows);
    after
}

#[test]
fn ten_guid_components_preserve_full_bounds_masks_and_last_component_uniqueness() {
    for code in [5, 6] {
        for reversed in [false, true] {
            for ignored in [false, true] {
                let case = source(code, reversed, ignored, true);
                let free = if ignored { 124_u16 } else { 110_u16 };
                assert_eq!(
                    &case.before[14 * PAGE + 2..14 * PAGE + 4],
                    &free.to_le_bytes()
                );
                assert_eq!(
                    &case.before[12 * PAGE + 2..12 * PAGE + 4],
                    &852_u16.to_le_bytes()
                );
                let inserted = Row {
                    id: 20,
                    members: std::array::from_fn(|component| Some(guid(230, component))),
                };
                let after = run(&case, &inserted);
                assert_eq!(after.len(), 18 * PAGE);
                assert_eq!(after[14 * PAGE], 3);
                assert_eq!(
                    &after[14 * PAGE + 2..14 * PAGE + 4],
                    &3418_u16.to_le_bytes()
                );
                assert_eq!(
                    &after[14 * PAGE + 20..14 * PAGE + 24],
                    &16_u32.to_le_bytes()
                );
                assert_eq!(
                    &after[16 * PAGE + 12..16 * PAGE + 16],
                    &17_u32.to_le_bytes()
                );
                assert_eq!(
                    &after[17 * PAGE + 16..17 * PAGE + 20],
                    &16_u32.to_le_bytes()
                );
                assert_eq!(&after[12 * PAGE + 2..12 * PAGE + 4], &682_u16.to_le_bytes());

                let case = source(code, reversed, ignored, false);
                let last = case.columns.last().unwrap().0 - 1;
                let mut inserted = case.rows[0].0.clone();
                inserted.id = 4;
                inserted.members[last] = Some(guid(7, last));
                let after = run(&case, &inserted);
                assert_eq!(after.len(), case.before.len());
                assert_eq!(after[14 * PAGE], 4);
                let mut duplicate = case.rows[1].0.clone();
                duplicate.id = 4;
                let result = original::capture(&case.before).preview_ace16_insert_rows(
                    "Items",
                    &[RowInsert::new(duplicate.values())],
                    &mut original::budget(),
                    &|| false,
                );
                assert_eq!(
                    result.err(),
                    Some(EditError::Inspect(InspectError::IndexConstraint(
                        IndexConstraintViolation::DuplicateKey
                    )))
                );
            }
        }
    }
}
