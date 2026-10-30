//! Independent literal SOURCE and complete new-index expectations; no mutation API.
mod index;

use crate::{
    table::mutation::insert::tests::multiple_indexes::model as frames, test_support::base,
};
use std::collections::BTreeSet;
use tablecraft_testkit::access as raw;

pub(super) const PAGE: usize = 4096;
pub(super) const SOURCE_PAGES: u32 = 24;

#[derive(Clone, Copy, Debug)]
pub(super) enum Population {
    Empty,
    Distinct,
    NullableDuplicate,
    Pressure,
}

pub(super) const POPULATIONS: [Population; 4] = [
    Population::Empty,
    Population::Distinct,
    Population::NullableDuplicate,
    Population::Pressure,
];

#[derive(Clone)]
pub(super) struct Row {
    pub id: i32,
    pub score: Option<i32>,
    pub guard: i32,
    pub reference: u32,
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub rows: Vec<Row>,
    pub old_streams: [Vec<Vec<u8>>; 2],
    pub new_stream: Vec<Vec<u8>>,
    pub name: &'static str,
    pub descending: bool,
    pub new_counter: u32,
    pub new_pages: Vec<u32>,
    pub source_definition_length: usize,
    pub expected_definition_length: usize,
}

fn scores(population: Population) -> Vec<Option<i32>> {
    match population {
        Population::Empty => Vec::new(),
        Population::Distinct => vec![Some(-1), Some(0), Some(1)],
        Population::NullableDuplicate => vec![None, Some(-7), Some(-7), Some(0), None, Some(7)],
        Population::Pressure => (0..600)
            .map(|position| (position % 10 != 0).then_some(position - 300))
            .collect(),
    }
}

fn data(owner: u32, records: Vec<Vec<u8>>) -> Vec<u8> {
    let length = records.iter().map(Vec::len).sum::<usize>();
    let count = records.len();
    let rows = records
        .into_iter()
        .map(|record| (0, record))
        .collect::<Vec<_>>();
    let mut page = raw::data_page(owner, &rows);
    raw::put16(&mut page, 2, (PAGE - 14 - 2 * count - length) as u16);
    page
}

fn record(row: &Row) -> Vec<u8> {
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(row.score.map_or([0xa5; 4], i32::to_le_bytes));
    bytes.extend(row.guard.to_le_bytes());
    bytes.push(5 | (u8::from(row.score.is_some()) << 1));
    assert_eq!(bytes.len(), 15);
    bytes
}

fn source_definition(rows: usize, old: &[u8]) -> Vec<u8> {
    let mut definition = base::definition(
        &[("Id", 4, 0, 4), ("Score", 4, 4, 4), ("Guard", 4, 8, 4)],
        rows as u32,
        11 << 8,
    );
    definition.copy_within(63..168, 87);
    definition[63..87].fill(0);
    raw::put32(&mut definition, 67, 1000);
    raw::put32(&mut definition, 79, 2000);
    raw::put32(&mut definition, 47, 4);
    raw::put32(&mut definition, 51, 2);
    raw::put32(&mut definition, 59, (11 << 8) | 1);
    // Reuse the literal existing four-alias/two-owner framing, not a producer.
    definition[192..498].copy_from_slice(&old[192..498]);
    raw::put16(&mut definition, 244 + 4, 2);
    raw::put16(&mut definition, 498, 0xffff);
    raw::put32(&mut definition, 8, 500);
    raw::put16(&mut definition, 2, 4096 - 500 - 8);
    definition[500..].fill(0xd3);
    definition
}

fn sentinel(before: &mut [u8]) {
    let mut definition = base::definition(&[("Marker", 4, 0, 4)], 1, 19 << 8);
    raw::put32(&mut definition, 59, (19 << 8) | 1);
    raw::put16(&mut definition, 102, 0xffff);
    raw::put32(&mut definition, 8, 104);
    raw::put16(&mut definition, 2, 4096 - 104 - 8);
    frames::install(before, 18, &definition);
    let mut record = 1_u16.to_le_bytes().to_vec();
    record.extend(999_i32.to_le_bytes());
    record.push(1);
    frames::install(before, 20, &data(18, vec![record]));
    frames::map_page(before, 19, vec![frames::map([20]), frames::map([20])]);

    let catalog = [
        raw::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        raw::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        raw::record(10, 0, 1, 0, b"\xff\xfeItems"),
        raw::record(18, 0, 1, 0, b"\xff\xfeSentinel"),
    ];
    frames::install(before, 4, &data(2, catalog.into()));
    raw::put32(before, 2 * PAGE + 16, 4);
}

fn expected_definition(
    before: &[u8],
    name: &str,
    descending: bool,
    counter: u32,
) -> (Vec<u8>, usize) {
    let original = &before[10 * PAGE..11 * PAGE];
    let mut active = original[..63].to_vec();
    active.extend(&original[63..87]);
    let mut count = [0; 12];
    raw::put32(&mut count, 4, counter);
    active.extend(count);
    active.extend(&original[87..192]);
    active.extend(&original[192..296]);
    active.extend(index::physical(descending));
    active.extend(&original[296..408]);
    active.extend(index::logical());
    active.extend(&original[408..498]);
    let name = name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    active.extend((name.len() as u16).to_le_bytes());
    active.extend(name);
    active.extend([0xff, 0xff]);
    let length = active.len();
    raw::put32(&mut active, 8, length as u32);
    raw::put16(&mut active, 2, (PAGE - length - 8) as u16);
    raw::put32(&mut active, 47, 5);
    raw::put32(&mut active, 51, 3);
    let mut result = original.to_vec();
    result[..length].copy_from_slice(&active);
    (result, length)
}

pub(super) fn source(code: u8, population: Population, descending: bool) -> Case {
    let seed = frames::source(code, false, false).before;
    let mut before = seed.clone();
    before.resize(SOURCE_PAGES as usize * PAGE, 0);
    let rows = scores(population)
        .into_iter()
        .enumerate()
        .map(|(position, score)| Row {
            id: position as i32 + 1,
            score,
            guard: (position % 7) as i32,
            reference: ([12, 16, 17][position / 200] << 8) | (position % 200) as u32,
        })
        .collect::<Vec<_>>();
    frames::install(
        &mut before,
        10,
        &source_definition(rows.len(), &seed[10 * PAGE..11 * PAGE]),
    );
    let data_pages = if rows.is_empty() {
        vec![12]
    } else {
        [12, 16, 17][..rows.len().div_ceil(200)].to_vec()
    };
    for (position, &page) in data_pages.iter().enumerate() {
        let start = position * 200;
        let end = (start + 200).min(rows.len());
        frames::install(
            &mut before,
            page,
            &data(10, rows[start..end].iter().map(record).collect()),
        );
    }
    frames::map_page(
        &mut before,
        11,
        vec![
            frames::map(data_pages.clone()),
            frames::map(data_pages.clone()),
        ],
    );
    let old_streams = [
        index::entries(&rows, 0, false),
        index::entries(&rows, 2, false),
    ];
    for (position, stream) in old_streams.iter().enumerate() {
        let mut leaf = frames::leaf(None, stream, 0, 0, 0);
        let active = 3616 - usize::from(u16::from_le_bytes(leaf[2..4].try_into().unwrap()));
        leaf[480 + active..].fill(0xc0 + position as u8);
        frames::install(&mut before, 13 + position as u32, &leaf);
    }
    frames::map_page(&mut before, 15, vec![frames::map([13]), frames::map([14])]);
    sentinel(&mut before);
    let used = (0..16)
        .chain(data_pages)
        .chain(18..21)
        .collect::<BTreeSet<_>>();
    frames::map_page(
        &mut before,
        1,
        vec![frames::map((0..512).filter(|page| !used.contains(page)))],
    );

    let name = if descending {
        "ScoreLookupDesc"
    } else {
        "ScoreLookupAsc"
    };
    let new_stream = index::entries(&rows, 1, descending);
    let new_counter = rows
        .iter()
        .map(|row| row.score)
        .collect::<BTreeSet<_>>()
        .len() as u32;
    let pressure = matches!(population, Population::Pressure);
    let new_pages = if pressure {
        vec![24, 25, 26, 27]
    } else {
        vec![24, 25]
    };
    let mut expected = before.clone();
    expected.resize((if pressure { 28 } else { 26 }) * PAGE, 0);
    let (definition, expected_definition_length) =
        expected_definition(&before, name, descending, new_counter);
    frames::install(&mut expected, 10, &definition);
    let members: &[u32] = if pressure { &[25, 26, 27] } else { &[25] };
    frames::install(
        &mut expected,
        24,
        &data(0, vec![index::owned_map(25, members)]),
    );
    if pressure {
        let middle = new_stream.len() / 2;
        frames::install(
            &mut expected,
            26,
            &frames::leaf(None, &new_stream[..middle], 0, 0, 27),
        );
        frames::install(
            &mut expected,
            27,
            &frames::leaf(None, &new_stream[middle..], 0, 26, 0),
        );
        frames::install(
            &mut expected,
            25,
            &frames::root(&vec![0; PAGE], 26, 27, &new_stream[middle - 1]),
        );
    } else {
        frames::install(&mut expected, 25, &frames::leaf(None, &new_stream, 0, 0, 0));
    }
    let final_used = used
        .into_iter()
        .chain(new_pages.iter().copied())
        .collect::<BTreeSet<_>>();
    frames::map_page(
        &mut expected,
        1,
        vec![frames::map(
            (0..512).filter(|page| !final_used.contains(page)),
        )],
    );

    Case {
        before,
        expected,
        rows,
        old_streams,
        new_stream,
        name,
        descending,
        new_counter,
        new_pages,
        source_definition_length: 500,
        expected_definition_length,
    }
}
