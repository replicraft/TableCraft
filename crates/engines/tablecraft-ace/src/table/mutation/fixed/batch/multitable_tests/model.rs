//! Two genuine table owners and one independent, definition-ordered image model.
use super::super::split_tests::model::{self as original, Row, Shape};
use crate::table::mutation::insert::tests::multiple_indexes::model as frames;
use std::ops::Range;
use tablecraft_testkit::access as raw;

pub(in crate::table) const PAGE: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::table) enum Mode {
    Full,
    Clear,
    LowFitted,
    HighFitted,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::table) struct Settings {
    pub code: u8,
    pub descending: bool,
    pub unique: bool,
    pub forwarded: bool,
    pub mode: Mode,
}

pub(in crate::table) struct Table {
    pub name: &'static str,
    pub definition: u32,
    pub roots: [u32; 2],
    pub data: Vec<u32>,
    pub index_map: u32,
    pub original: Vec<Row>,
    pub final_rows: Vec<Row>,
    pub history: [u32; 2],
    pub target: Option<u32>,
    pub fitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::table) struct Added {
    pub page: u32,
    pub definition: u32,
    pub physical: usize,
}

pub(in crate::table) struct Case {
    pub settings: Settings,
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub tables: [Table; 2],
    pub added: Vec<Added>,
    pub eof: u32,
}

pub(in crate::table) fn settings() -> Vec<Settings> {
    let mut result = Vec::new();
    for code in [5, 6] {
        for descending in [false, true] {
            for forwarded in [false, true] {
                for unique in [false, true] {
                    for mode in [Mode::Full, Mode::Clear] {
                        result.push(Settings {
                            code,
                            descending,
                            unique,
                            forwarded,
                            mode,
                        });
                    }
                }
                for mode in [Mode::LowFitted, Mode::HighFitted] {
                    result.push(Settings {
                        code,
                        descending,
                        unique: false,
                        forwarded,
                        mode,
                    });
                }
            }
        }
    }
    assert_eq!(result.len(), 48);
    result
}

pub(in crate::table) fn span(bytes: &[u8], reference: u32) -> Range<usize> {
    let base = (reference >> 8) as usize * PAGE;
    let slot = (reference & 255) as usize;
    let word = |offset| {
        usize::from(u16::from_le_bytes(
            bytes[offset..offset + 2].try_into().unwrap(),
        )) & 0x1fff
    };
    let start = word(base + 14 + slot * 2);
    let end = if slot == 0 {
        PAGE
    } else {
        word(base + 12 + slot * 2)
    };
    base + start..base + end
}

fn component(value: Option<i32>, descending: bool) -> Vec<u8> {
    let mut result = match value {
        None => vec![0],
        Some(value) => {
            let mut result = vec![127];
            result.extend(((value as u32) ^ 0x8000_0000).to_be_bytes());
            result
        }
    };
    if descending {
        for byte in &mut result {
            *byte = !*byte;
        }
    }
    result
}

fn entries(rows: &[Row], physical: usize, descending: bool) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    for row in rows {
        if physical == 1 && row.left.is_none() && row.right.is_none() {
            continue;
        }
        let mut key = if physical == 0 {
            component(row.left, descending)
        } else {
            let mut key = component(row.right, !descending);
            key.extend(component(row.left, descending));
            key
        };
        let length = key.len();
        key.extend(row.reference.to_be_bytes());
        result.push((key, length, row.reference));
    }
    result.sort_by(|a, b| (&a.0[..a.1], a.2).cmp(&(&b.0[..b.1], b.2)));
    result.into_iter().map(|entry| entry.0).collect()
}

fn fits(keys: &[Vec<u8>]) -> bool {
    if keys.is_empty() {
        return true;
    }
    let common = keys.iter().skip(1).fold(keys[0].len(), |prefix, key| {
        keys[0][..prefix]
            .iter()
            .zip(key)
            .take_while(|(a, b)| a == b)
            .count()
    });
    keys.iter().map(Vec::len).sum::<usize>() - common * (keys.len() - 1) <= 3616
}

fn signed(ordinal: usize) -> i32 {
    if ordinal.is_multiple_of(2) {
        i32::MIN + ordinal as i32 * 17
    } else {
        i32::MAX - ordinal as i32 * 17
    }
}

fn owner(mut page: Vec<u8>, definition: u32) -> Vec<u8> {
    raw::put32(&mut page, 4, definition);
    page
}

fn catalog(bytes: &mut [u8]) {
    let mut rows = (0..3)
        .map(|slot| (0, bytes[span(bytes, (4 << 8) | slot)].to_vec()))
        .collect::<Vec<_>>();
    let position = rows[2]
        .1
        .windows(5)
        .position(|part| part == b"Items")
        .unwrap();
    rows[2].1[position..position + 5].copy_from_slice(b"Zulu_");
    let mut second = rows[2].1.clone();
    raw::put32(&mut second, 2, 20);
    second[position..position + 5].copy_from_slice(b"Alpha");
    rows.push((0, second));
    raw::put32(bytes, 2 * PAGE + 16, 4);
    let mut page = raw::data_page(2, &rows);
    let used = rows.iter().map(|(_, row)| row.len() + 2).sum::<usize>();
    raw::put16(&mut page, 2, (PAGE - 14 - used) as u16);
    frames::install(bytes, 4, &page);
}

fn record(bytes: &mut [u8], table: &Table, position: usize) {
    let row = &table.final_rows[position];
    let reference = if position == 0 {
        table.target.unwrap_or(row.reference)
    } else {
        row.reference
    };
    let range = span(bytes, reference);
    if let Some(value) = row.left {
        bytes[range.start + 6..range.start + 10].copy_from_slice(&value.to_le_bytes());
    }
    if let Some(value) = row.right {
        bytes[range.start + 10..range.start + 14].copy_from_slice(&value.to_le_bytes());
    }
    bytes[range.end - 1] =
        1 | (u8::from(row.left.is_some()) << 1) | (u8::from(row.right.is_some()) << 2);
}

pub(in crate::table) fn source(settings: Settings) -> Case {
    assert!(
        !settings.unique || matches!(settings.mode, Mode::Full | Mode::Clear),
        "Repeated fitted families are a nonunique control"
    );
    // Reuse the actual independent source/schema owner, never its expected image.
    let template = original::source(
        settings.code,
        Shape::Mixed,
        settings.descending,
        settings.unique,
        false,
    );
    let mut before = template.before.clone();
    before.resize(if settings.forwarded { 32 } else { 30 } * PAGE, 0);
    catalog(&mut before);
    let mut tables = [(10_u32, "Zulu_"), (20_u32, "Alpha")].map(|(definition, name)| {
        let shift = definition - 10;
        let mut rows = template.original.clone();
        for row in &mut rows {
            row.reference += shift << 8;
        }
        let fitted = matches!(
            (settings.mode, definition),
            (Mode::LowFitted, 10) | (Mode::HighFitted, 20)
        );
        let mut final_rows = rows.clone();
        for (position, row) in final_rows.iter_mut().enumerate() {
            if fitted {
                if position < 2 {
                    row.left = Some(42);
                    row.right = Some(777);
                }
            } else if settings.mode == Mode::Clear && position >= 300 {
                row.left = None;
                row.right = None;
            } else {
                row.left = Some(signed(position));
                row.right = Some(signed(position + 500));
            }
        }
        Table {
            name,
            definition,
            roots: [13 + shift, 14 + shift],
            data: vec![12 + shift, 16 + shift, 17 + shift],
            index_map: 15 + shift,
            original: rows,
            final_rows,
            history: [700, if fitted { 801 } else { 1100 }],
            target: settings.forwarded.then_some((30 + shift / 10) << 8),
            fitted,
        }
    });
    for table in &mut tables {
        let shift = table.definition - 10;
        let base = table.definition as usize * PAGE;
        let mut definition = template.before[10 * PAGE..11 * PAGE].to_vec();
        raw::put32(&mut definition, 55, (11 + shift) << 8);
        raw::put32(&mut definition, 59, ((11 + shift) << 8) | 1);
        for physical in 0..2 {
            raw::put32(
                &mut definition,
                226 + physical * 52,
                (table.index_map << 8) | physical as u32,
            );
            raw::put32(&mut definition, 230 + physical * 52, table.roots[physical]);
        }
        before[base..base + PAGE].copy_from_slice(&definition);
        for (original_page, relocated) in [12, 16, 17].into_iter().zip(&table.data) {
            frames::install(
                &mut before,
                *relocated,
                &owner(
                    template.before[original_page * PAGE..(original_page + 1) * PAGE].to_vec(),
                    table.definition,
                ),
            );
        }
        if let Some(target) = table.target {
            let source = span(&before, table.original[0].reference);
            let body = before[source.clone()].to_vec();
            let mut hidden = raw::data_page(table.definition, &[(0x8000, body)]);
            raw::put16(&mut hidden, 2, 4065);
            frames::install(&mut before, target >> 8, &hidden);
            before[source.clone()].fill(0);
            raw::put32(&mut before, source.start, target);
            let directory = (table.original[0].reference >> 8) as usize * PAGE + 14;
            let word = u16::from_le_bytes(before[directory..directory + 2].try_into().unwrap());
            raw::put16(&mut before, directory, word | 0x4000);
            table.data.push(target >> 8);
        }
        frames::map_page(
            &mut before,
            11 + shift,
            vec![
                frames::map(table.data.clone()),
                frames::map([12 + shift, 16 + shift, 17 + shift]),
            ],
        );
        frames::map_page(
            &mut before,
            table.index_map,
            vec![frames::map([table.roots[0]]), frames::map([table.roots[1]])],
        );
        for physical in 0..2 {
            let keys = entries(&table.original, physical, settings.descending);
            assert!(fits(&keys));
            let mut leaf = owner(frames::leaf(None, &keys, 0, 0, 0), table.definition);
            let active = 3616 - usize::from(u16::from_le_bytes(leaf[2..4].try_into().unwrap()));
            leaf[480 + active..].fill(0x90 + physical as u8);
            frames::install(&mut before, table.roots[physical], &leaf);
        }
    }
    for (page, definition, reference) in [(28_u32, 10_u32, 12_u32 << 8), (29, 20, 22 << 8)] {
        let mut key = vec![0];
        key.extend(reference.to_be_bytes());
        let mut leaf = owner(frames::leaf(None, &[key], 0, 0, 0), definition);
        leaf[485..].fill(0xa0 + page as u8);
        frames::install(&mut before, page, &leaf);
    }
    let eof = (before.len() / PAGE) as u32;
    frames::map_page(
        &mut before,
        1,
        vec![frames::map([18, 19].into_iter().chain(eof..512))],
    );
    let mut expected = before.clone();
    let mut next = eof;
    let mut added = Vec::new();
    for table in &tables {
        for position in 0..table.final_rows.len() {
            record(&mut expected, table, position);
        }
        let mut members = [vec![table.roots[0]], vec![table.roots[1]]];
        for (physical, owned) in members.iter_mut().enumerate() {
            let keys = entries(&table.final_rows, physical, settings.descending);
            let root = table.roots[physical];
            let old = &before[root as usize * PAGE..(root as usize + 1) * PAGE];
            if table.fitted {
                assert!(fits(&keys));
                frames::install(
                    &mut expected,
                    root,
                    &owner(frames::leaf(Some(old), &keys, 0, 0, 0), table.definition),
                );
            } else {
                assert!(
                    !fits(&keys),
                    "The final population overflows even after complete prefix repair"
                );
                let right = next;
                let left = next + 1;
                next += 2;
                expected.resize(next as usize * PAGE, 0);
                let middle = keys.len() / 2;
                frames::install(
                    &mut expected,
                    left,
                    &owner(
                        frames::leaf(None, &keys[..middle], 0, 0, right),
                        table.definition,
                    ),
                );
                frames::install(
                    &mut expected,
                    right,
                    &owner(
                        frames::leaf(None, &keys[middle..], 0, left, 0),
                        table.definition,
                    ),
                );
                frames::install(
                    &mut expected,
                    root,
                    &owner(
                        frames::root(old, left, right, &keys[middle - 1]),
                        table.definition,
                    ),
                );
                for page in [right, left] {
                    added.push(Added {
                        page,
                        definition: table.definition,
                        physical,
                    });
                    owned.push(page);
                }
            }
        }
        raw::put32(
            &mut expected,
            table.definition as usize * PAGE + 79,
            table.history[1],
        );
        frames::map_page(
            &mut expected,
            table.index_map,
            members.into_iter().map(frames::map).collect(),
        );
    }
    frames::map_page(
        &mut expected,
        1,
        vec![frames::map([18, 19].into_iter().chain(next..512))],
    );
    Case {
        settings,
        before,
        expected,
        tables,
        added,
        eof,
    }
}
