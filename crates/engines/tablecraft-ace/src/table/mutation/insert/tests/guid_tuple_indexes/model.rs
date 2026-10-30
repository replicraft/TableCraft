//! Literal fixed records and complete physical GUID/tuple image expectations.
use std::collections::BTreeSet;
use tablecraft_core::values::{Guid, Value};
use tablecraft_testkit::access as raw;
use tablecraft_testkit::access::multiple_indexes as frames;
use tablecraft_testkit::access::mutation_fixture as base;

const PAGE: usize = 4096;

#[derive(Clone, Copy)]
pub(in crate::table::mutation::insert) enum Shape {
    Guid,
    GuidThenScore,
    ScoreThenGuid,
}

#[derive(Clone, Copy)]
pub(in crate::table::mutation::insert) struct Settings {
    pub(in crate::table::mutation::insert) shape: Shape,
    pub(in crate::table::mutation::insert) guid_descending: bool,
    pub(in crate::table::mutation::insert) score_descending: bool,
    pub(in crate::table::mutation::insert) unique: bool,
    pub(in crate::table::mutation::insert) ignored: bool,
    pub(in crate::table::mutation::insert) required: bool,
    pub(in crate::table::mutation::insert) primary_second: bool,
}

impl Settings {
    pub(in crate::table::mutation::insert) fn ordinary(shape: Shape) -> Self {
        Self {
            shape,
            guid_descending: false,
            score_descending: true,
            unique: false,
            ignored: false,
            required: false,
            primary_second: false,
        }
    }

    fn columns(self, physical: usize) -> Vec<(u16, bool)> {
        if physical == usize::from(self.primary_second) {
            return vec![(0, false)];
        }
        let guid = (1, self.guid_descending);
        let score = (2, self.score_descending);
        match self.shape {
            Shape::Guid => vec![guid],
            Shape::GuidThenScore => vec![guid, score],
            Shape::ScoreThenGuid => vec![score, guid],
        }
    }
}

#[derive(Clone)]
pub(in crate::table::mutation::insert) struct Row {
    pub id: i32,
    pub guid: Option<[u8; 16]>,
    pub score: Option<i32>,
}

impl Row {
    pub(super) fn values(&self) -> Vec<Value> {
        vec![
            Value::Int32(self.id),
            self.guid
                .map_or(Value::Null, |bytes| Value::Guid(Guid::from_bytes(bytes))),
            self.score.map_or(Value::Null, Value::Int32),
        ]
    }
}

pub(in crate::table::mutation::insert) struct Case {
    pub(in crate::table::mutation::insert) before: Vec<u8>,
    pub rows: Vec<(Row, u32)>,
    pub data: Vec<u32>,
    pub settings: Settings,
    pub histories: [u32; 2],
}

pub(super) fn guid(seed: u8) -> [u8; 16] {
    [seed, 1, 2, 3, 4, 5, 6, 7, 0x80, 9, 10, 11, 12, 13, 14, 0xff]
}

fn record(row: &Row) -> Vec<u8> {
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    let mut stored = row.guid.unwrap_or([0; 16]);
    stored[..4].reverse();
    stored[4..6].reverse();
    stored[6..8].reverse();
    bytes.extend(stored);
    bytes.extend(row.score.unwrap_or(0).to_le_bytes());
    bytes.push(1 | (u8::from(row.guid.is_some()) << 1) | (u8::from(row.score.is_some()) << 2));
    bytes
}

fn guid_component(value: Option<[u8; 16]>, descending: bool) -> Vec<u8> {
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

fn integer_component(value: Option<i32>, descending: bool) -> Vec<u8> {
    let mut bytes = if let Some(value) = value {
        let mut bytes = vec![127];
        bytes.extend(((value as u32) ^ 0x80000000).to_be_bytes());
        bytes
    } else {
        vec![0]
    };
    if descending {
        for byte in &mut bytes {
            *byte = !*byte;
        }
    }
    bytes
}

fn key(row: &Row, settings: Settings, physical: usize) -> Option<Vec<u8>> {
    let columns = settings.columns(physical);
    let primary = physical == usize::from(settings.primary_second);
    let all_null = columns.iter().all(|(column, _)| match column {
        0 => false,
        1 => row.guid.is_none(),
        2 => row.score.is_none(),
        _ => unreachable!(),
    });
    if !primary && settings.ignored && all_null {
        return None;
    }
    let mut bytes = Vec::new();
    for (column, descending) in columns {
        bytes.extend(match column {
            0 => integer_component(Some(row.id), descending),
            1 => guid_component(row.guid, descending),
            2 => integer_component(row.score, descending),
            _ => unreachable!(),
        });
    }
    Some(bytes)
}

fn entries(rows: &[(Row, u32)], settings: Settings, physical: usize) -> Vec<Vec<u8>> {
    let mut entries = rows
        .iter()
        .filter_map(|(row, reference)| {
            let mut bytes = key(row, settings, physical)?;
            bytes.extend(reference.to_be_bytes());
            Some(bytes)
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

pub(in crate::table::mutation::insert) fn source(
    code: u8,
    settings: Settings,
    pressure: bool,
) -> Case {
    let mut before = frames::source(code, false, false).before;
    before.resize(if pressure { 18 } else { 16 } * PAGE, 0);
    let records = if pressure {
        assert!(matches!(settings.shape, Shape::Guid));
        (0..158)
            .map(|position| Row {
                id: position + 1,
                guid: (position < 157).then(|| guid(position as u8)),
                score: Some(position),
            })
            .collect::<Vec<_>>()
    } else {
        vec![
            Row {
                id: 1,
                guid: Some(guid(16)),
                score: Some(10),
            },
            Row {
                id: 2,
                guid: Some(guid(
                    if settings.unique && matches!(settings.shape, Shape::Guid) {
                        32
                    } else {
                        16
                    },
                )),
                score: Some(20),
            },
            Row {
                id: 3,
                guid: settings.required.then(|| guid(48)),
                score: settings.required.then_some(30),
            },
        ]
    };
    let primary = usize::from(settings.primary_second);
    let mut histories = [if pressure { 329 } else { 29 }; 2];
    histories[primary] = if pressure { 217 } else { 17 };
    let mut definition = base::definition(
        &[("ID", 4, 0, 4), ("Group", 15, 4, 16), ("Score", 4, 20, 4)],
        records.len() as u32,
        11 << 8,
    );
    definition.copy_within(63..168, 87);
    definition[63..87].fill(0);
    raw::put32(&mut definition, 47, 4);
    raw::put32(&mut definition, 51, 2);
    raw::put32(&mut definition, 59, (11 << 8) | 1);
    definition[192..408].fill(0);
    for (physical, history) in histories.iter().enumerate() {
        raw::put32(&mut definition, 67 + 12 * physical, *history);
        let at = 192 + 52 * physical;
        for component in 0..10 {
            raw::put16(&mut definition, at + 4 + 3 * component, 0xffff);
        }
        for (component, (column, descending)) in settings.columns(physical).into_iter().enumerate()
        {
            raw::put16(&mut definition, at + 4 + 3 * component, column);
            definition[at + 6 + 3 * component] = u8::from(!descending);
        }
        raw::put32(&mut definition, at + 34, (15 << 8) | physical as u32);
        raw::put32(&mut definition, at + 38, 13 + physical as u32);
        definition[at + 46] = if physical == primary {
            0x89
        } else {
            0x80 | u8::from(settings.unique)
                | (u8::from(settings.ignored) << 1)
                | (u8::from(settings.required) << 3)
        };
    }
    let mut names = 408;
    for (logical, name) in ["PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias"]
        .into_iter()
        .enumerate()
    {
        let at = 296 + logical * 28;
        let physical = if logical < 2 { primary } else { 1 - primary };
        raw::put32(&mut definition, at + 4, logical as u32);
        raw::put32(&mut definition, at + 8, physical as u32);
        definition[at + 23] = u8::from(logical == 0);
        let text = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        raw::put16(&mut definition, names, text.len() as u16);
        names += 2;
        definition[names..names + text.len()].copy_from_slice(&text);
        names += text.len();
    }
    raw::put16(&mut definition, names, 0xffff);
    frames::install(&mut before, 10, &definition);
    let data = if pressure { vec![12, 17] } else { vec![12] };
    let mut rows = Vec::new();
    let chunk_size = records.len().div_ceil(data.len());
    for (page, chunk) in data.iter().zip(records.chunks(chunk_size)) {
        let encoded = chunk.iter().map(|row| (0, record(row))).collect::<Vec<_>>();
        let mut body = raw::data_page(10, &encoded);
        raw::put16(
            &mut body,
            2,
            (PAGE - 14 - 2 * chunk.len() - 27 * chunk.len()) as u16,
        );
        frames::install(&mut before, *page, &body);
        rows.extend(
            chunk
                .iter()
                .cloned()
                .enumerate()
                .map(|(slot, row)| (row, (*page << 8) | slot as u32)),
        );
    }
    frames::map_page(
        &mut before,
        11,
        vec![frames::map(data.clone()), frames::map(data.clone())],
    );
    frames::map_page(&mut before, 15, vec![frames::map([13]), frames::map([14])]);
    let eof = (before.len() / PAGE) as u32;
    frames::map_page(&mut before, 1, vec![frames::map(eof..512)]);
    for physical in 0..2 {
        frames::install(
            &mut before,
            13 + physical as u32,
            &frames::leaf(None, &entries(&rows, settings, physical), 0, 0, 0),
        );
    }
    Case {
        before,
        rows,
        data,
        settings,
        histories,
    }
}

pub(super) fn expected(case: &Case, requests: &[Row]) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let mut rows = case.rows.clone();
    let owned = case.data.clone();
    let hints = owned.clone();
    for row in requests {
        let body = record(row);
        let page = *hints.iter().max().unwrap();
        let start = page as usize * PAGE;
        let count = u16::from_le_bytes(bytes[start + 12..start + 14].try_into().unwrap()) as usize;
        let end = if count == 0 {
            PAGE
        } else {
            u16::from_le_bytes(
                bytes[start + 12 + 2 * count..start + 14 + 2 * count]
                    .try_into()
                    .unwrap(),
            ) as usize
        };
        assert!(count < 255 && end >= 14 + 2 * (count + 1) + body.len());
        let at = end - body.len();
        bytes[start + at..start + end].copy_from_slice(&body);
        raw::put16(&mut bytes, start + 14 + 2 * count, at as u16);
        raw::put16(&mut bytes, start + 12, (count + 1) as u16);
        raw::put16(&mut bytes, start + 2, (at - 14 - 2 * (count + 1)) as u16);
        rows.push((row.clone(), (page << 8) | count as u32));
    }
    let mut members = [vec![13], vec![14]];
    for (physical, members) in members.iter_mut().enumerate() {
        let final_entries = entries(&rows, case.settings, physical);
        let old = &case.before[(13 + physical) * PAGE..(14 + physical) * PAGE];
        let original_entries = entries(&case.rows, case.settings, physical);
        if final_entries == original_entries {
            continue;
        }
        if frames::framing(&final_entries, 0).1 <= 3616 {
            frames::install(
                &mut bytes,
                13 + physical as u32,
                &frames::leaf(Some(old), &final_entries, 0, 0, 0),
            );
        } else {
            let right = (bytes.len() / PAGE) as u32;
            let left = right + 1;
            bytes.resize(bytes.len() + 2 * PAGE, 0);
            let middle = final_entries.len() / 2;
            frames::install(
                &mut bytes,
                right,
                &frames::leaf(None, &final_entries[middle..], 0, left, 0),
            );
            frames::install(
                &mut bytes,
                left,
                &frames::leaf(None, &final_entries[..middle], 0, 0, right),
            );
            frames::install(
                &mut bytes,
                13 + physical as u32,
                &frames::root(old, left, right, &final_entries[middle - 1]),
            );
            members.extend([right, left]);
        }
        let mut known = case
            .rows
            .iter()
            .filter_map(|(row, _)| key(row, case.settings, physical))
            .collect::<BTreeSet<_>>();
        let delta = requests
            .iter()
            .filter_map(|row| key(row, case.settings, physical))
            .filter(|key| known.insert(key.clone()))
            .count();
        raw::put32(
            &mut bytes,
            10 * PAGE + 67 + 12 * physical,
            case.histories[physical] + delta as u32,
        );
    }
    raw::put32(&mut bytes, 10 * PAGE + 16, rows.len() as u32);
    frames::map_page(&mut bytes, 11, vec![frames::map(owned), frames::map(hints)]);
    frames::map_page(
        &mut bytes,
        15,
        members.into_iter().map(frames::map).collect(),
    );
    let eof = (bytes.len() / PAGE) as u32;
    frames::map_page(&mut bytes, 1, vec![frames::map(eof..512)]);
    bytes
}
