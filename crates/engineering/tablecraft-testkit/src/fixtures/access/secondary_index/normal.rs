//! Normal secondary-index images over genuine retained sources; no engine codecs.
pub(super) mod definition;
pub mod dropping;
pub mod history;
pub(super) mod tree;
/// Independent existing-tree effects after ordinary CREATE INDEX.
pub mod updating;

use crate::fixtures::access::{
    canonical_retained_add_table::{currency::mixed, extend_global_map, fields, populated},
    packed_record::append_record,
    put16, put32,
};
use std::collections::BTreeSet;

const PAGE: usize = 4096;

/// Explicit populations exercise physical topology rather than a preview quota.
#[derive(Clone, Copy, Debug)]
pub enum Population {
    Empty,
    NullableDuplicate,
    ThreeLeaves,
    MultiLevel,
}

/// Literal source membership; ROW references never come from an engine reader.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub score: Option<i32>,
    pub reference: u32,
}

/// Original and complete independently authored new-index image.
pub struct Source {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub table: &'static str,
    pub column: &'static str,
    pub index_name: &'static str,
    pub target_definition: u32,
    pub root: u32,
    pub index_pages: Vec<u32>,
    pub leaf_pages: usize,
    pub levels: usize,
    pub map_host: u32,
    pub rows: Vec<Row>,
    pub descending: bool,
}

fn scores(population: Population) -> Vec<Option<i32>> {
    let count = match population {
        Population::Empty => return Vec::new(),
        Population::NullableDuplicate => {
            return vec![None, Some(-7), Some(-7), Some(0), None, Some(7)];
        }
        Population::ThreeLeaves => 3_000,
        Population::MultiLevel => 200_000,
    };
    (0..count)
        .map(|position| {
            if position % 17 == 0 {
                None
            } else {
                // Paired duplicates spread across the complete signed32 range; the
                // expected tree cannot collapse into a narrow repeated-key prefix.
                let ranked = (position / 2) as u64;
                let bits = (ranked * u64::from(u32::MAX) / (count / 2 - 1) as u64) as u32;
                Some((bits ^ 0x8000_0000) as i32)
            }
        })
        .collect()
}

/// A fresh keyless LONG field, real MSysDb catalog and protected existing siblings.
pub fn source(population: Population, descending: bool) -> Source {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / PAGE) as u32;
    let scores = scores(population);
    let before = populated::numbers(&seed, "Archive", &scores);
    let rows = scores
        .into_iter()
        .enumerate()
        .map(|(ordinal, score)| Row {
            score,
            // The independent populated fixture starts a new DATA page after
            // 255 records; this is its packing policy, not a native slot limit.
            reference: ((definition + 1 + (ordinal / 255) as u32) << 8) | (ordinal % 255) as u32,
        })
        .collect::<Vec<_>>();
    // Check page-boundary ROW identities against literal physical row counts,
    // independently of the tree encoder and the native reader.
    for (ordinal, page_offset, slot) in [
        (254, 0, 254),
        (255, 1, 0),
        (256, 1, 1),
        (510, 2, 0),
        (2_040, 8, 0),
    ] {
        if let Some(row) = rows.get(ordinal) {
            let page = definition + 1 + page_offset;
            assert_eq!(row.reference, (page << 8) | slot);
            let start = page as usize * PAGE;
            let count = u16::from_le_bytes(before[start + 12..start + 14].try_into().unwrap());
            assert!(u32::from(count) > slot);
        }
    }
    let result = finish(
        before,
        definition,
        "Archive",
        "CopiedScore",
        0,
        rows,
        descending,
    );
    if matches!(population, Population::ThreeLeaves) {
        assert!(result.leaf_pages > 2);
    }
    if matches!(population, Population::MultiLevel) {
        assert!(
            result.levels >= 3,
            "literal source must create a genuinely multilevel tree"
        );
    }
    result
}

/// Generated ID plus selected LONG and untouched Currency/Unicode Text companions.
pub fn protected_companion(descending: bool) -> Source {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / PAGE) as u32;
    let mut before = mixed::append_table(&seed, "CompanionValues");
    for _ in 0..3 {
        before = mixed::append(&before, definition);
    }
    let scores = [None, Some(-7), Some(-7)];
    for (slot, ((score, amount), text)) in scores
        .into_iter()
        .zip([Some(12_345), None, Some(-99_000)])
        .zip([Some("日本🦀"), None, Some("")])
        .enumerate()
    {
        before = mixed::update_number(&before, definition, slot, score);
        before = mixed::update_currency(&before, definition, slot, amount);
        before = mixed::update_text(&before, definition, slot, text);
    }
    let rows = scores
        .into_iter()
        .enumerate()
        .map(|(slot, score)| Row {
            score,
            reference: ((definition + 1) << 8) | slot as u32,
        })
        .collect();
    finish(
        before,
        definition,
        "CompanionValues",
        "Score",
        1,
        rows,
        descending,
    )
}

pub(super) fn finish(
    before: Vec<u8>,
    owner: u32,
    table: &'static str,
    column: &'static str,
    native_number: u16,
    rows: Vec<Row>,
    descending: bool,
) -> Source {
    let logical = u32::from_le_bytes(
        before[owner as usize * PAGE + 47..owner as usize * PAGE + 51]
            .try_into()
            .unwrap(),
    );
    finish_named(
        before,
        owner,
        Target {
            table,
            column,
            native_number,
            descending,
        },
        rows,
        Identity {
            index_name: "ScoreLookup",
            logical,
        },
    )
}

struct Target {
    table: &'static str,
    column: &'static str,
    native_number: u16,
    descending: bool,
}

struct Identity {
    index_name: &'static str,
    logical: u32,
}

fn finish_named(
    before: Vec<u8>,
    owner: u32,
    target: Target,
    rows: Vec<Row>,
    identity: Identity,
) -> Source {
    let Target {
        table,
        column,
        native_number,
        descending,
    } = target;
    let Identity {
        index_name,
        logical,
    } = identity;
    let first = (before.len() / PAGE) as u32;
    let tree = tree::build(&rows, first, owner, descending);
    let map_host = first + tree.pages.len() as u32;
    let index_pages = (first..map_host).collect::<Vec<_>>();
    let map_bytes = 5 + 64_usize.max(index_pages.len().div_ceil(8));
    // These explicit populations fit one represented inline map row. This is a
    // fixture geometry assertion, not an admission ceiling for native creation.
    assert!(map_bytes <= PAGE - 16);
    let mut map = vec![0; map_bytes];
    put32(&mut map, 1, first);
    for offset in 0..index_pages.len() {
        map[5 + offset / 8] |= 1 << (offset % 8);
    }
    let mut container = vec![0; PAGE];
    container[..2].copy_from_slice(&[1, 1]);
    put16(&mut container, 2, (PAGE - 14) as u16);
    append_record(&mut container, &map);
    let distinct = rows
        .iter()
        .map(|row| row.score)
        .collect::<BTreeSet<_>>()
        .len();
    let declaration_request = definition::NewIndex {
        name: index_name,
        column: native_number,
        descending,
        usage: map_host << 8,
        root: tree.root,
        distinct: distinct as u32,
    };
    let raw = &before[owner as usize * PAGE..(owner as usize + 1) * PAGE];
    let count = u32::from_le_bytes(raw[47..51].try_into().unwrap());
    let declaration = if logical == count {
        definition::append(raw, declaration_request)
    } else {
        definition::append_with_logical(raw, declaration_request, logical)
    };
    // NO_CLONE: The independent AFTER image coexists with its immutable SOURCE;
    // neither is a production plan or shares writable payload with the caller.
    let mut expected = before.to_vec();
    expected[owner as usize * PAGE..(owner as usize + 1) * PAGE].copy_from_slice(&declaration);
    for page in tree.pages {
        expected.extend(page);
    }
    expected.extend(container);
    extend_global_map(&mut expected, before.len() / PAGE);
    assert_eq!(expected.len(), (map_host as usize + 1) * PAGE);
    for page in 0..before.len() / PAGE {
        if page != 1 && page != owner as usize {
            assert_eq!(
                &expected[page * PAGE..(page + 1) * PAGE],
                &before[page * PAGE..(page + 1) * PAGE]
            );
        }
    }
    Source {
        before,
        expected,
        table,
        column,
        index_name,
        target_definition: owner,
        root: tree.root,
        index_pages,
        leaf_pages: tree.leaf_pages,
        levels: tree.levels,
        map_host,
        rows,
        descending,
    }
}
