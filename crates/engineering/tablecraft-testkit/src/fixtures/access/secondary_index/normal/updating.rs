//! Literal existing-tree UPDATE expectations over actual ordinary CREATE images.
use super::{Population, Row, Source};
use crate::fixtures::access::{
    canonical_retained_add_table::{fields, populated},
    put16, raw_forwarding_chain,
};

const PAGE: usize = 4096;
const ENTRIES: usize = 480;
const CAPACITY: usize = PAGE - ENTRIES;

/// Original row ordinal and its independently authored selected-field values.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub row: usize,
    pub expected: Option<i32>,
    pub value: Option<i32>,
}

/// The CREATE input/output stays available to exercise real writer composition.
pub struct Case {
    pub creation: Source,
    pub expected: Vec<u8>,
    pub cells: Vec<Cell>,
}

/// Nullable/repeated root leaves and large retained topologies, in both directions.
pub fn source(population: Population, descending: bool) -> Case {
    let created = super::source(population, descending);
    let cells = match population {
        Population::Empty => Vec::new(),
        Population::NullableDuplicate => [Some(7), None, Some(7), Some(-99), None, Some(7)]
            .into_iter()
            .enumerate()
            .map(|(row, value)| Cell {
                row,
                expected: created.rows[row].score,
                value,
            })
            .collect(),
        Population::ThreeLeaves | Population::MultiLevel => vec![
            Cell {
                row: 1,
                expected: created.rows[1].score,
                value: created.rows[2].score,
            },
            Cell {
                row: 2,
                expected: created.rows[2].score,
                value: created.rows[1].score,
            },
        ],
    };
    finish(created, 0, cells)
}

/// Every original participates in a retained tree with over16,384 requests.
/// Rotating the complete key multiset preserves existing leaf capacities while
/// changing original ROW membership, including duplicate and NULL ties.
pub fn many_selected(descending: bool) -> Case {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / PAGE) as u32;
    let values = (0..16_385_i32)
        .map(|ordinal| (ordinal % 17 != 0).then_some(ordinal / 2))
        .collect::<Vec<_>>();
    let before = populated::numbers(&seed, "IndexedBatchValues", &values);
    let rows = values
        .into_iter()
        .enumerate()
        .map(|(ordinal, score)| Row {
            score,
            reference: ((definition + 1 + (ordinal / 255) as u32) << 8) | (ordinal % 255) as u32,
        })
        .collect();
    let created = super::finish(
        before,
        definition,
        "IndexedBatchValues",
        "CopiedScore",
        0,
        rows,
        descending,
    );
    assert!(created.leaf_pages > 2);
    let cells = created
        .rows
        .iter()
        .enumerate()
        .map(|(row, original)| Cell {
            row,
            expected: original.score,
            value: created.rows[(row + 1) % created.rows.len()].score,
        })
        .collect();
    finish(created, 0, cells)
}

/// A preexisting local UNIQUE declaration uses the independently recorded
/// physical flags0x81, or0x89 for UNIQUE+Required. This is not CREATE UNIQUE.
pub fn unique(descending: bool, required: bool) -> Case {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / PAGE) as u32;
    let values = [Some(1), Some(2), Some(3)];
    let before = populated::numbers(&seed, "UniqueValues", &values);
    let rows = values
        .into_iter()
        .enumerate()
        .map(|(slot, score)| Row {
            score,
            reference: ((definition + 1) << 8) | slot as u32,
        })
        .collect();
    let mut created = super::finish(
        before,
        definition,
        "UniqueValues",
        "CopiedScore",
        0,
        rows,
        descending,
    );
    // One12B counter, one25B column,24B UTF16 name, then52B physical owner.
    // Multiple-index raw fixtures independently use the same0x81/0x89 law.
    let flag = definition as usize * PAGE + 63 + 12 + 25 + 24 + 46;
    assert_eq!(created.expected[flag], 0x80);
    created.expected[flag] = if required { 0x89 } else { 0x81 };
    finish(
        created,
        0,
        vec![
            Cell {
                row: 0,
                expected: Some(1),
                value: if required { Some(4) } else { None },
            },
            Cell {
                row: 2,
                expected: Some(3),
                value: Some(5),
            },
        ],
    )
}

/// Generated primary INDEX, LAST, Currency and Unicode siblings remain protected.
pub fn protected_companion(descending: bool) -> Case {
    let created = super::protected_companion(descending);
    let cells = [Some(5), None, Some(5)]
        .into_iter()
        .enumerate()
        .map(|(row, value)| Cell {
            row,
            expected: created.rows[row].score,
            value,
        })
        .collect();
    finish(created, 1, cells)
}

/// Complete six-row SQL `CLng(Nz(CopiedScore,0)+1)` expectation.
pub fn incremented(descending: bool) -> Case {
    let created = super::source(Population::NullableDuplicate, descending);
    let cells = created
        .rows
        .iter()
        .enumerate()
        .map(|(row, original)| Cell {
            row,
            expected: original.score,
            value: Some(original.score.unwrap_or(0) + 1),
        })
        .collect();
    finish(created, 0, cells)
}

/// One original nullable row materializes while every other key stays unchanged.
pub fn single(descending: bool) -> Case {
    let created = super::source(Population::NullableDuplicate, descending);
    finish(
        created,
        0,
        vec![Cell {
            row: 0,
            expected: None,
            value: Some(1),
        }],
    )
}

/// The same supplied expression protects generated and typed sibling fields.
pub fn protected_incremented(descending: bool) -> Case {
    let created = super::protected_companion(descending);
    let cells = created
        .rows
        .iter()
        .enumerate()
        .map(|(row, original)| Cell {
            row,
            expected: original.score,
            value: Some(original.score.unwrap_or(0) + 1),
        })
        .collect();
    finish(created, 1, cells)
}

/// Twenty forwarding links retain the original logical ROW in both indexes.
/// The physical chain is authored independently after ordinary CREATE completes.
pub fn forwarded(descending: bool) -> Case {
    let mut case = protected_companion(descending);
    let page = case.creation.target_definition + 1;
    let updated = raw_forwarding_chain::record(&case.expected, page, 0).to_vec();
    let original = raw_forwarding_chain::record(&case.creation.expected, page, 0);
    // Forwarding must preserve the old terminal's ignored historical bytes.
    // Only the new terminal receives the selected-field replacement.
    case.expected = raw_forwarding_chain::replace_terminal(&case.expected, page, 0, original);
    case.creation.expected = raw_forwarding_chain::extend(&case.creation.expected, page, 0, 20);
    case.expected = raw_forwarding_chain::extend(&case.expected, page, 0, 20);
    case.expected = raw_forwarding_chain::replace_terminal(&case.expected, page, 0, &updated);
    case
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

pub(super) fn finish(created: Source, native: usize, cells: Vec<Cell>) -> Case {
    let base = created.target_definition as usize * PAGE;
    let physical = long(&created.expected, base + 51) as usize;
    let field = base + 63 + physical * 12 + native * 25;
    assert_eq!(created.expected[field], 4);
    assert_eq!(word(&created.expected, field + 5), native);
    let mut expected = created.expected.to_vec();
    let mut rows = created.rows.to_vec();
    for cell in &cells {
        let row = &mut rows[cell.row];
        assert_eq!(row.score, cell.expected);
        let range = raw_forwarding_chain::terminal_range(
            &expected,
            row.reference >> 8,
            (row.reference & 255) as usize,
        );
        let offset = 2 + word(&created.expected, field + 21);
        let record = &mut expected[range];
        let count = word(record, 0);
        assert!(native < count);
        let mask = record.len() - count.div_ceil(8);
        assert!(offset + 4 <= mask);
        let present = record[mask + native / 8] & (1 << (native % 8)) != 0;
        assert_eq!(present, cell.expected.is_some());
        if let Some(original) = cell.expected {
            assert_eq!(&record[offset..offset + 4], &original.to_le_bytes());
        }
        if let Some(value) = cell.value {
            record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            record[mask + native / 8] |= 1 << (native % 8);
        } else {
            record[mask + native / 8] &= !(1 << (native % 8));
        }
        row.score = cell.value;
    }
    let mut keys = rows
        .iter()
        .map(|row| {
            let mut wire = match row.score {
                Some(value) => {
                    let mut key = vec![127];
                    key.extend(((value as u32) ^ 0x8000_0000).to_be_bytes());
                    key
                }
                None => vec![0],
            };
            if created.descending {
                for byte in &mut wire {
                    *byte = !*byte;
                }
            }
            wire.extend(row.reference.to_be_bytes());
            wire
        })
        .collect::<Vec<_>>();
    keys.sort();
    let mut remaining = keys.as_slice();
    rewrite(
        &created.expected,
        &mut expected,
        created.root,
        &mut remaining,
    );
    assert!(remaining.is_empty());
    // Metadata, usage maps and every other page retain their complete old image.
    assert_eq!(
        &expected[base..base + PAGE],
        &created.expected[base..base + PAGE]
    );
    assert_eq!(expected.len(), created.expected.len());
    Case {
        creation: created,
        expected,
        cells,
    }
}

fn original_entries(page: &[u8]) -> Vec<Vec<u8>> {
    let prefix = word(page, 24);
    let mut result: Vec<Vec<u8>> = Vec::new();
    let mut start = 0;
    for end in 1..=CAPACITY {
        if page[27 + end / 8] & (1 << (end % 8)) == 0 {
            continue;
        }
        let mut wire = if result.is_empty() {
            Vec::new()
        } else {
            result[0][..prefix].to_vec()
        };
        wire.extend_from_slice(&page[ENTRIES + start..ENTRIES + end]);
        result.push(wire);
        start = end;
    }
    assert_eq!(start, CAPACITY - word(page, 2));
    result
}

fn rewrite(
    before: &[u8],
    after: &mut [u8],
    number: u32,
    remaining: &mut &[Vec<u8>],
) -> Option<Vec<u8>> {
    let base = number as usize * PAGE;
    let original = &before[base..base + PAGE];
    let old = original_entries(original);
    let mut entries = Vec::new();
    let last = match original[0] {
        4 => {
            let (selected, rest) = remaining.split_at(old.len());
            entries.extend(selected.iter().map(|wire| wire.to_vec()));
            *remaining = rest;
            selected.last().map(|wire| wire.to_vec())
        }
        3 => {
            let mut last = None;
            for entry in &old {
                let child = u32::from_be_bytes(entry[entry.len() - 4..].try_into().unwrap());
                let mut maximum = rewrite(before, after, child, remaining).unwrap();
                maximum.extend(child.to_be_bytes());
                entries.push(maximum);
            }
            let tail = long(original, 20);
            if tail != 0 {
                last = rewrite(before, after, tail, remaining);
            }
            last
        }
        other => panic!("unexpected literal INDEX kind {other}"),
    };
    repack(original, &mut after[base..base + PAGE], &entries);
    last
}

fn repack(original: &[u8], after: &mut [u8], entries: &[Vec<u8>]) {
    let mut common = entries.first().map_or(0, Vec::len);
    if entries.len() > 1 && original[0] == 3 {
        common -= 4;
    }
    for entry in entries.iter().skip(1) {
        let bound = entry.len() - usize::from(original[0] == 3) * 4;
        common = common.min(bound);
        common = entries[0][..common]
            .iter()
            .zip(entry)
            .take_while(|(left, right)| left == right)
            .count();
    }
    let full = entries.iter().map(Vec::len).sum::<usize>();
    let saved = entries.len().saturating_sub(1);
    let mut prefix = word(original, 24).min(common);
    if full - saved * prefix > CAPACITY {
        prefix = common;
    }
    let length = full - saved * prefix;
    assert!(length <= CAPACITY, "literal existing topology must fit");
    after[27..ENTRIES].fill(0);
    let mut end = 0;
    for (position, wire) in entries.iter().enumerate() {
        let suffix = &wire[if position == 0 { 0 } else { prefix }..];
        after[ENTRIES + end..ENTRIES + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        after[27 + end / 8] |= 1 << (end % 8);
    }
    put16(after, 2, (CAPACITY - length) as u16);
    put16(after, 24, prefix as u16);
}
