//! Eligible streams omit NULL without changing the complete logical ROW universe.
use super::super::multiple_indexes::model as frames;
use std::collections::BTreeSet;
use tablecraft_testkit::access as raw;

pub const PAGE: usize = 4096;

#[derive(Clone, Copy)]
pub enum Shape {
    Mixed,
    Empty,
    Node,
    Growth,
}

pub struct Case {
    pub source: frames::Case,
    pub primary_last: bool,
    pub node: bool,
    pub owned: Vec<u32>,
    pub hints: Vec<u32>,
    pub index_pages: [Vec<u32>; 2],
}

fn entries(rows: &[(frames::Row, u32)], secondary: bool, descending: bool) -> Vec<Vec<u8>> {
    let mut result = rows
        .iter()
        .filter_map(|(row, reference)| {
            let value = if secondary { row.group? } else { row.id };
            let mut wire = vec![if secondary && descending { 128 } else { 127 }];
            wire.extend(
                ((value as u32) ^ 0x80000000)
                    .to_be_bytes()
                    .map(|byte| if secondary && descending { !byte } else { byte }),
            );
            wire.extend(reference.to_be_bytes());
            Some(wire)
        })
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn data_page(rows: &[frames::Row]) -> Vec<u8> {
    let records = rows
        .iter()
        .map(|row| (0, frames::record(row)))
        .collect::<Vec<_>>();
    let mut page = raw::data_page(10, &records);
    let free =
        PAGE - 14 - 2 * records.len() - records.iter().map(|(_, body)| body.len()).sum::<usize>();
    raw::put16(&mut page, 2, free as u16);
    page
}

pub fn source(code: u8, shape: Shape, descending: bool, unique: bool, primary_last: bool) -> Case {
    let large = matches!(shape, Shape::Node | Shape::Growth);
    let mut source = frames::source(code, large, unique);
    source.descending = descending;
    if matches!(shape, Shape::Empty) {
        for (row, _) in &mut source.rows {
            row.group = None;
        }
        let records = source
            .rows
            .iter()
            .map(|(row, _)| row.clone())
            .collect::<Vec<_>>();
        frames::install(&mut source.before, 12, &data_page(&records));
    }
    source.before[10 * PAGE + 250] = u8::from(!descending);
    source.before[10 * PAGE + 290] = if unique { 0x83 } else { 0x82 };
    let eligible = entries(&source.rows, true, descending);
    frames::install(
        &mut source.before,
        14,
        &frames::leaf(None, &eligible, 0, 0, 0),
    );
    let mut index_pages = [vec![13], vec![14]];
    if matches!(shape, Shape::Node) {
        let old = source.before[14 * PAGE..15 * PAGE].to_vec();
        let middle = eligible.len() / 2;
        source.before.resize(20 * PAGE, 0);
        frames::install(
            &mut source.before,
            18,
            &frames::leaf(None, &eligible[..middle], 0, 0, 19),
        );
        frames::install(
            &mut source.before,
            19,
            &frames::leaf(None, &eligible[middle..], 0, 18, 0),
        );
        frames::install(
            &mut source.before,
            14,
            &frames::root(&old, 18, 19, &eligible[middle - 1]),
        );
        index_pages[1].extend([18, 19]);
        frames::map_page(
            &mut source.before,
            15,
            vec![frames::map([13]), frames::map([14, 18, 19])],
        );
        frames::map_page(&mut source.before, 1, vec![frames::map(20..512)]);
    }
    if primary_last {
        let primary = source.before[10 * PAGE + 192..10 * PAGE + 244].to_vec();
        let secondary = source.before[10 * PAGE + 244..10 * PAGE + 296].to_vec();
        source.before[10 * PAGE + 192..10 * PAGE + 244].copy_from_slice(&secondary);
        source.before[10 * PAGE + 244..10 * PAGE + 296].copy_from_slice(&primary);
        for logical in 0..4 {
            raw::put32(
                &mut source.before,
                10 * PAGE + 296 + 28 * logical + 8,
                if logical < 2 { 1 } else { 0 },
            );
        }
        raw::put32(&mut source.before, 10 * PAGE + 67, source.histories[1]);
        raw::put32(&mut source.before, 10 * PAGE + 79, source.histories[0]);
    }
    let owned = source.data.clone();
    let hints = source.data.clone();
    Case {
        source,
        primary_last,
        node: matches!(shape, Shape::Node),
        owned,
        hints,
        index_pages,
    }
}

pub fn forward(mut case: Case, slot: usize) -> Case {
    let data = &case.source.before[12 * PAGE..13 * PAGE];
    let start = usize::from(u16::from_le_bytes(
        data[14 + 2 * slot..16 + 2 * slot].try_into().unwrap(),
    ));
    let end = if slot == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            data[12 + 2 * slot..14 + 2 * slot].try_into().unwrap(),
        ))
    };
    let original = data[start..end].to_vec();
    let target = (case.source.before.len() / PAGE) as u32;
    case.source
        .before
        .resize(case.source.before.len() + PAGE, 0);
    let free = PAGE - 16 - original.len();
    let mut hidden = raw::data_page(10, &[(0x8000, original)]);
    raw::put16(&mut hidden, 2, free as u16);
    frames::install(&mut case.source.before, target, &hidden);
    let at = 12 * PAGE + start;
    case.source.before[at..12 * PAGE + end].fill(0);
    raw::put32(&mut case.source.before, at, target << 8);
    raw::put16(
        &mut case.source.before,
        12 * PAGE + 14 + 2 * slot,
        start as u16 | 0x4000,
    );
    case.owned.push(target);
    frames::map_page(
        &mut case.source.before,
        11,
        vec![
            frames::map(case.owned.clone()),
            frames::map(case.hints.clone()),
        ],
    );
    frames::map_page(
        &mut case.source.before,
        1,
        vec![frames::map(target + 1..512)],
    );
    case
}

pub fn expected(case: &Case, requests: &[frames::Row]) -> Vec<u8> {
    let mut bytes = case.source.before.clone();
    let mut rows = case.source.rows.clone();
    let mut hints = case.hints.clone();
    let mut owned = case.owned.clone();
    for row in requests {
        let record = frames::record(row);
        let candidate = *hints.iter().max().unwrap();
        let base = candidate as usize * PAGE;
        let free = usize::from(u16::from_le_bytes(
            bytes[base + 2..base + 4].try_into().unwrap(),
        ));
        let page = if free >= record.len() + 2 {
            candidate
        } else {
            hints.retain(|number| *number != candidate);
            let next = (bytes.len() / PAGE) as u32;
            bytes.resize(bytes.len() + PAGE, 0);
            frames::install(&mut bytes, next, &raw::data_page(10, &[]));
            hints.push(next);
            owned.push(next);
            next
        };
        let base = page as usize * PAGE;
        let count = usize::from(u16::from_le_bytes(
            bytes[base + 12..base + 14].try_into().unwrap(),
        ));
        let end = if count == 0 {
            PAGE
        } else {
            usize::from(
                u16::from_le_bytes(
                    bytes[base + 12 + 2 * count..base + 14 + 2 * count]
                        .try_into()
                        .unwrap(),
                ) & 0x1fff,
            )
        };
        let begin = end - record.len();
        bytes[base + begin..base + end].copy_from_slice(&record);
        raw::put16(&mut bytes, base + 14 + 2 * count, begin as u16);
        raw::put16(&mut bytes, base + 12, (count + 1) as u16);
        raw::put16(&mut bytes, base + 2, (begin - 14 - 2 * (count + 1)) as u16);
        rows.push((row.clone(), (page << 8) | count as u32));
    }
    let mut members = case.index_pages.clone();
    for physical in 0..2 {
        let logical = if case.primary_last {
            1 - physical
        } else {
            physical
        };
        let secondary = logical == 1;
        if secondary && requests.iter().all(|row| row.group.is_none()) {
            continue;
        }
        assert!(
            !secondary || !case.node,
            "this literal NODE case only admits omitted new NULL keys"
        );
        let entries = entries(&rows, secondary, case.source.descending);
        let root = 13 + logical;
        let original = &case.source.before[root * PAGE..(root + 1) * PAGE];
        let seed = usize::from(u16::from_le_bytes(original[24..26].try_into().unwrap()));
        if frames::framing(&entries, seed).1 <= 3616 {
            frames::install(
                &mut bytes,
                root as u32,
                &frames::leaf(Some(original), &entries, seed, 0, 0),
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
                root as u32,
                &frames::root(original, left, right, &entries[middle - 1]),
            );
            members[logical].extend([right, left]);
        }
    }
    let mut families = case
        .source
        .rows
        .iter()
        .filter_map(|(row, _)| row.group)
        .collect::<BTreeSet<_>>();
    let mut distinct = 0;
    for row in requests {
        if let Some(group) = row.group {
            distinct += u32::from(families.insert(group));
        }
    }
    raw::put32(&mut bytes, 10 * PAGE + 16, rows.len() as u32);
    let primary_counter = if case.primary_last { 79 } else { 67 };
    let secondary_counter = if case.primary_last { 67 } else { 79 };
    raw::put32(
        &mut bytes,
        10 * PAGE + primary_counter,
        case.source.histories[0] + requests.len() as u32,
    );
    raw::put32(
        &mut bytes,
        10 * PAGE + secondary_counter,
        case.source.histories[1] + distinct,
    );
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

/// Rebuild actual non-NULL Group payloads and keys before declaring Required.
pub fn required_source(code: u8, ignored_primary: bool) -> Case {
    let mut case = source(code, Shape::Mixed, false, true, false);
    case.source.rows[2].0.group = Some(30);
    let records = case
        .source
        .rows
        .iter()
        .map(|(row, _)| row.clone())
        .collect::<Vec<_>>();
    frames::install(&mut case.source.before, 12, &data_page(&records));
    let keys = entries(&case.source.rows, true, false);
    frames::install(
        &mut case.source.before,
        14,
        &frames::leaf(None, &keys, 0, 0, 0),
    );
    if ignored_primary {
        case.source.before[10 * PAGE + 238] = 0x8b;
    } else {
        case.source.before[10 * PAGE + 290] = 0x8b;
    }
    case
}

/// Each alternative remains a well-framed leaf, but violates eligible membership.
pub fn malformed_membership(mut case: Case, variant: usize) -> Case {
    let mut keys = entries(&case.source.rows, true, false);
    match variant {
        0 => {
            let mut null = vec![0];
            null.extend(((12_u32 << 8) | 2).to_be_bytes());
            keys.push(null);
            keys.sort();
        }
        1 => {
            keys.pop();
        }
        2 => {
            keys[0][5..9].copy_from_slice(&((12_u32 << 8) | 99).to_be_bytes());
        }
        _ => unreachable!(),
    }
    keys.sort();
    frames::install(
        &mut case.source.before,
        14,
        &frames::leaf(None, &keys, 0, 0, 0),
    );
    case
}
