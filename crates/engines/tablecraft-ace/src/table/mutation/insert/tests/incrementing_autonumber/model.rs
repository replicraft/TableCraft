//! Independent generated-ID records, metadata and complete physical-index images.
use super::super::{model as original, multiple_indexes::model as frames};
use crate::test_support::base;
use std::collections::BTreeSet;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as raw;

pub const PAGE: usize = 4096;
pub const DEFINITION: usize = 10 * PAGE;
pub const GENERATED_FLAGS: usize = DEFINITION + 87 + 25 + 15;

#[derive(Clone, Copy, Debug)]
pub enum State {
    Fresh,
    Negative,
    Minimum,
    HighestDeleted,
    AllDeleted,
    HighWater,
    NearMaximum,
    Maximum,
    Collision,
    Pressure,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub earlier: i32,
    pub id: i32,
    pub score: Option<i32>,
}

impl Row {
    pub fn values(&self) -> Vec<Value> {
        vec![
            Value::Int32(self.earlier),
            Value::Int32(self.id),
            self.score.map_or(Value::Null, Value::Int32),
        ]
    }

    pub fn companions(&self) -> Vec<Value> {
        vec![
            Value::Int32(self.earlier),
            self.score.map_or(Value::Null, Value::Int32),
        ]
    }
}

pub struct Case {
    pub before: Vec<u8>,
    pub physical_rows: Vec<(Row, u32, bool)>,
    pub data: Vec<u32>,
    pub histories: [u32; 2],
    pub last: i32,
}

fn record(row: &Row, old: bool) -> Vec<u8> {
    let mut bytes = vec![3, 0];
    bytes.extend(row.earlier.to_le_bytes());
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(
        row.score
            .map_or(if old { [0xa5; 4] } else { [0; 4] }, i32::to_le_bytes),
    );
    bytes.push(3 | (u8::from(row.score.is_some()) << 2));
    bytes
}

fn wires(rows: &[(Row, u32)], secondary: bool) -> Vec<Vec<u8>> {
    let mut entries = rows
        .iter()
        .map(|(row, reference)| {
            let value = if secondary { row.score } else { Some(row.id) };
            let mut key = if let Some(value) = value {
                let mut key = vec![127];
                key.extend(((value as u32) ^ 0x80000000).to_be_bytes());
                key
            } else {
                vec![0]
            };
            key.extend(reference.to_be_bytes());
            key
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

pub fn live(case: &Case) -> Vec<Row> {
    case.physical_rows
        .iter()
        .filter(|(_, _, deleted)| !deleted)
        .map(|(row, _, _)| row.clone())
        .collect()
}

pub fn source(code: u8, state: State) -> Case {
    let mut bytes = original::source(code, false, false, None);
    let count = match state {
        State::HighestDeleted | State::AllDeleted => 3,
        State::HighWater => 2,
        State::Collision => 1,
        State::Pressure => 600,
        _ => 0,
    };
    let last = match state {
        State::Fresh => 0,
        State::Negative => -2,
        State::Minimum => i32::MIN,
        State::HighestDeleted | State::AllDeleted | State::Collision => 3,
        State::HighWater => 260,
        State::NearMaximum => i32::MAX - 1,
        State::Maximum => i32::MAX,
        State::Pressure => 600,
    };
    let histories = match state {
        State::HighestDeleted | State::AllDeleted => [3, 3],
        State::HighWater => [260, 29],
        State::Collision => [1, 1],
        State::Pressure => [600, 1],
        _ => [0, 0],
    };
    let rows = (0..count)
        .map(|position| {
            let id = if matches!(state, State::Collision) {
                4
            } else {
                position + 1
            };
            let deleted = matches!(state, State::AllDeleted)
                || (matches!(state, State::HighestDeleted) && id == 3)
                || (matches!(state, State::Pressure) && id == 600);
            (
                Row {
                    earlier: 700 + id,
                    id,
                    score: if matches!(state, State::Pressure) {
                        Some(10)
                    } else if id == 3 {
                        None
                    } else {
                        Some(id * 10)
                    },
                },
                deleted,
            )
        })
        .collect::<Vec<_>>();
    let data = if matches!(state, State::Pressure) {
        vec![12, 16, 17]
    } else {
        vec![12]
    };
    if data.len() > 1 {
        bytes.resize(18 * PAGE, 0);
    }
    let fields = [("Earlier", 4, 0, 4), ("ID", 4, 4, 4), ("Score", 4, 8, 4)];
    let fields_end = 63
        + fields.len() * 25
        + fields
            .iter()
            .map(|(name, _, _, _)| 2 + name.len() * 2)
            .sum::<usize>();
    let mut definition = base::definition(
        &fields,
        rows.iter().filter(|(_, deleted)| !deleted).count() as u32,
        11 << 8,
    );
    definition.copy_within(63..fields_end, 87);
    definition[63..87].fill(0);
    definition[87 + 25 + 15] = 7;
    raw::put32(&mut definition, 20, last as u32);
    definition[24..28].copy_from_slice(&[1, 0, 0, 0]);
    raw::put32(&mut definition, 67, histories[0]);
    raw::put32(&mut definition, 79, histories[1]);
    raw::put32(&mut definition, 47, 4);
    raw::put32(&mut definition, 51, 2);
    raw::put32(&mut definition, 59, (11 << 8) | 1);
    let physical = fields_end + 24;
    definition[physical..].fill(0);
    for index in 0..2 {
        let at = physical + index * 52;
        for component in 0..10 {
            raw::put16(&mut definition, at + 4 + component * 3, 0xffff);
        }
        raw::put16(&mut definition, at + 4, (index + 1) as u16);
        definition[at + 6] = 1;
        raw::put32(&mut definition, at + 34, (15 << 8) | index as u32);
        raw::put32(&mut definition, at + 38, 13 + index as u32);
        definition[at + 46] = if index == 0 { 0x89 } else { 0x80 };
    }
    let logical = physical + 104;
    let mut names = logical + 4 * 28;
    for (number, name) in ["PrimaryID", "PrimaryAlias", "ScoreOrder", "ScoreAlias"]
        .into_iter()
        .enumerate()
    {
        let at = logical + number * 28;
        raw::put32(&mut definition, at + 4, number as u32);
        raw::put32(&mut definition, at + 8, (number / 2) as u32);
        definition[at + 23] = u8::from(number == 0);
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
    frames::install(&mut bytes, 10, &definition);
    let mut physical_rows = Vec::new();
    for (position, &page) in data.iter().enumerate() {
        let chunk = if data.len() == 1 {
            &rows[..]
        } else {
            &rows[position * 200..(position + 1) * 200]
        };
        let records = chunk
            .iter()
            .map(|(row, deleted)| (if *deleted { 0xc000 } else { 0 }, record(row, true)))
            .collect::<Vec<_>>();
        let mut body = raw::data_page(10, &records);
        raw::put16(&mut body, 2, (PAGE - 14 - records.len() * 17) as u16);
        frames::install(&mut bytes, page, &body);
        physical_rows.extend(
            chunk
                .iter()
                .enumerate()
                .map(|(slot, (row, deleted))| (row.clone(), (page << 8) | slot as u32, *deleted)),
        );
    }
    frames::map_page(
        &mut bytes,
        11,
        vec![frames::map(data.clone()), frames::map(data.clone())],
    );
    frames::map_page(&mut bytes, 15, vec![frames::map([13]), frames::map([14])]);
    let eof = (bytes.len() / PAGE) as u32;
    frames::map_page(&mut bytes, 1, vec![frames::map(eof..512)]);
    let live = physical_rows
        .iter()
        .filter(|(_, _, deleted)| !deleted)
        .map(|(row, reference, _)| (row.clone(), *reference))
        .collect::<Vec<_>>();
    for index in 0..2 {
        let entries = wires(&live, index == 1);
        let seed = if matches!(state, State::Pressure) {
            if index == 0 { 3 } else { 5 }
        } else {
            0
        };
        frames::install(
            &mut bytes,
            13 + index,
            &frames::leaf(None, &entries, seed, 0, 0),
        );
    }
    Case {
        before: bytes,
        physical_rows,
        data,
        histories,
        last,
    }
}

pub fn expected(case: &Case, requests: &[Row]) -> Vec<u8> {
    if requests.is_empty() {
        return case.before.clone();
    }
    let mut bytes = case.before.clone();
    let mut rows = case
        .physical_rows
        .iter()
        .filter(|(_, _, deleted)| !deleted)
        .map(|(row, reference, _)| (row.clone(), *reference))
        .collect::<Vec<_>>();
    let mut known = rows
        .iter()
        .map(|(row, _)| row.score)
        .collect::<BTreeSet<_>>();
    let mut added_families = 0;
    for (position, row) in requests.iter().enumerate() {
        assert_eq!(Some(row.id), case.last.checked_add(position as i32 + 1));
        added_families += u32::from(known.insert(row.score));
        let page = *case.data.last().unwrap() as usize * PAGE;
        let count = u16::from_le_bytes(bytes[page + 12..page + 14].try_into().unwrap()) as usize;
        let end = if count == 0 {
            PAGE
        } else {
            u16::from_le_bytes(
                bytes[page + 12 + count * 2..page + 14 + count * 2]
                    .try_into()
                    .unwrap(),
            ) as usize
                & 0x1fff
        };
        let record = record(row, false);
        let start = end - record.len();
        assert!(start >= 14 + 2 * (count + 1));
        bytes[page + start..page + end].copy_from_slice(&record);
        raw::put16(&mut bytes, page + 14 + count * 2, start as u16);
        raw::put16(&mut bytes, page + 12, (count + 1) as u16);
        raw::put16(&mut bytes, page + 2, (start - 14 - 2 * (count + 1)) as u16);
        rows.push((
            row.clone(),
            ((*case.data.last().unwrap()) << 8) | count as u32,
        ));
    }
    let mut members = [vec![13], vec![14]];
    for (index, members) in members.iter_mut().enumerate() {
        let entries = wires(&rows, index == 1);
        let old = &case.before[(13 + index) * PAGE..(14 + index) * PAGE];
        let seed = u16::from_le_bytes(old[24..26].try_into().unwrap()) as usize;
        if frames::framing(&entries, seed).1 <= 3616 {
            frames::install(
                &mut bytes,
                13 + index as u32,
                &frames::leaf(Some(old), &entries, seed, 0, 0),
            );
        } else {
            let right = (bytes.len() / PAGE) as u32;
            let left = right + 1;
            bytes.resize(bytes.len() + 2 * PAGE, 0);
            let middle = entries.len() / 2;
            frames::install(
                &mut bytes,
                right,
                &frames::leaf(None, &entries[middle..], 0, left, 0),
            );
            frames::install(
                &mut bytes,
                left,
                &frames::leaf(None, &entries[..middle], 0, 0, right),
            );
            frames::install(
                &mut bytes,
                13 + index as u32,
                &frames::root(old, left, right, &entries[middle - 1]),
            );
            members.extend([right, left]);
        }
    }
    raw::put32(&mut bytes, DEFINITION + 16, rows.len() as u32);
    raw::put32(
        &mut bytes,
        DEFINITION + 20,
        requests.last().unwrap().id as u32,
    );
    raw::put32(
        &mut bytes,
        DEFINITION + 67,
        case.histories[0] + requests.len() as u32,
    );
    raw::put32(
        &mut bytes,
        DEFINITION + 79,
        case.histories[1] + added_families,
    );
    frames::map_page(
        &mut bytes,
        15,
        members.into_iter().map(frames::map).collect(),
    );
    let eof = (bytes.len() / PAGE) as u32;
    frames::map_page(&mut bytes, 1, vec![frames::map(eof..512)]);
    bytes
}

/// Rebind only the independent earlier expected image, never producer output.
pub fn successor(case: &Case, added: &[Row]) -> Case {
    let before = expected(case, added);
    let page = *case.data.last().unwrap();
    let base = page as usize * PAGE;
    let first = u16::from_le_bytes(case.before[base + 12..base + 14].try_into().unwrap()) as u32;
    let mut physical_rows = case.physical_rows.clone();
    physical_rows.extend(
        added
            .iter()
            .enumerate()
            .map(|(position, row)| (row.clone(), (page << 8) | (first + position as u32), false)),
    );
    Case {
        histories: [
            u32::from_le_bytes(before[DEFINITION + 67..DEFINITION + 71].try_into().unwrap()),
            u32::from_le_bytes(before[DEFINITION + 79..DEFINITION + 83].try_into().unwrap()),
        ],
        last: added.last().map_or(case.last, |row| row.id),
        before,
        physical_rows,
        data: case.data.clone(),
    }
}
