//! Independent indexed originals/expectations. No production writer or index
//! encoder participates; the existing canonical source owns envelope and MR2.
use super::canonical_before_change::{self, Recipe as Program, Update};

const PAGE: usize = 4096;
const ROWS: [[i32; 2]; 3] = [[1, 10], [2, 20], [3, 30]];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Finite source identities; outcomes are literal, not learned from a writer.
pub enum Recipe {
    /// Single ascending Score key.
    Ascending,
    /// Single descending Score key.
    Descending,
    /// Score ascending followed by Id descending.
    Composite,
    /// Two logical names share one physical Score tree.
    Alias,
    /// Jointly final unique values swap across two original roots.
    UniqueSwap,
    /// A macro changes indexed Id while the user targets Score.
    MacroOnlyId,
    /// A real original user change is restored by Old.
    RestoreOriginal,
    /// Empty intents compile stored programs without evaluating a row.
    Empty,
    /// A final unique collision has no successful complete expectation.
    UniqueConflict,
}

/// Original and expected SOURCE qualification order.
pub const RECIPES: [Recipe; 9] = [
    Recipe::Ascending,
    Recipe::Descending,
    Recipe::Composite,
    Recipe::Alias,
    Recipe::UniqueSwap,
    Recipe::MacroOnlyId,
    Recipe::RestoreOriginal,
    Recipe::Empty,
    Recipe::UniqueConflict,
];

impl Recipe {
    /// Stable artifact identifier, independent of producer output.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
            Self::Composite => "composite",
            Self::Alias => "alias",
            Self::UniqueSwap => "unique-swap",
            Self::MacroOnlyId => "macro-only-id",
            Self::RestoreOriginal => "restore-original",
            Self::Empty => "empty",
            Self::UniqueConflict => "unique-conflict",
        }
    }
}

/// Complete original and independently encoded optional expected image.
pub struct Case {
    /// Original ten-page image with native MR2 and one physical tree.
    pub before: Vec<u8>,
    /// Complete expected image; absent only for the unique-conflict control.
    pub expected: Option<Vec<u8>>,
    /// User intent against original Score values, before any macro action.
    pub updates: Vec<Update>,
    /// Literal final rows, including the deliberately conflicting refusal rows.
    pub final_rows: [[i32; 2]; 3],
    /// Union of final changed original roots, not executed statement count.
    pub changed_rows: usize,
    /// Native column number and descending flag, in composite key order.
    pub components: Vec<(usize, bool)>,
    /// Physical UNIQUE declaration, separate from primary-key semantics.
    pub unique: bool,
    /// Whether a second logical name shares this physical tree.
    pub alias: bool,
    /// Exact unchanged catalog-owned MR2 Extra bytes.
    pub extra: Vec<u8>,
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn records(page: &[u8]) -> Vec<Vec<u8>> {
    let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
    (0..count)
        .map(|slot| {
            let word = 14 + 2 * slot;
            let start = usize::from(u16::from_le_bytes(page[word..word + 2].try_into().unwrap()));
            let end = if slot == 0 {
                PAGE
            } else {
                usize::from(u16::from_le_bytes(page[word - 2..word].try_into().unwrap()))
            };
            assert_eq!(start & 0xe000, 0);
            page[start..end].to_vec()
        })
        .collect()
}

fn packed_map_rows(rows: &[Vec<u8>]) -> Vec<u8> {
    let mut page = vec![0; PAGE];
    page[..2].copy_from_slice(&[1, 1]);
    put16(&mut page, 12, rows.len() as u16);
    let mut end = PAGE;
    for (slot, row) in rows.iter().enumerate() {
        end -= row.len();
        page[end..end + row.len()].copy_from_slice(row);
        put16(&mut page, 14 + 2 * slot, end as u16);
    }
    put16(&mut page, 2, (end - 14 - 2 * rows.len()) as u16);
    page
}

fn definition(original: &[u8], components: &[(usize, bool)], unique: bool, alias: bool) -> Vec<u8> {
    assert_eq!(u32::from_le_bytes(original[8..12].try_into().unwrap()), 133);
    assert_eq!(&original[47..55], &[0; 8]);
    let mut active = original[..63].to_vec();
    // One physical counter block; history3 is retained after equal-cardinality updates.
    active.extend_from_slice(&[0; 12]);
    put32(&mut active, 67, 3);
    active.extend_from_slice(&original[63..113]);
    active.extend_from_slice(&original[113..131]);
    assert_eq!(active.len(), 143);
    let mut physical = vec![0; 52];
    put32(&mut physical, 0, 1923);
    for component in 0..10 {
        put16(&mut physical, 4 + 3 * component, 0xffff);
    }
    for (position, &(column, descending)) in components.iter().enumerate() {
        put16(&mut physical, 4 + 3 * position, column as u16);
        physical[6 + 3 * position] = u8::from(!descending);
    }
    put32(&mut physical, 34, (3 << 8) | 16);
    put32(&mut physical, 38, 9);
    physical[46] = if unique { 0x81 } else { 0x80 };
    active.extend_from_slice(&physical);
    let count = if alias { 2 } else { 1 };
    for logical in 0..count {
        let mut descriptor = vec![0; 28];
        put32(&mut descriptor, 0, 1625);
        put32(&mut descriptor, 4, logical);
        put32(&mut descriptor, 8, 0);
        put32(&mut descriptor, 13, u32::MAX);
        active.extend_from_slice(&descriptor);
    }
    for name in ["EventKey", "EventAlias"].into_iter().take(count as usize) {
        let bytes = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        active.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        active.extend_from_slice(&bytes);
    }
    active.extend_from_slice(&[0xff, 0xff]);
    let length = active.len();
    put32(&mut active, 8, length as u32);
    put32(&mut active, 47, count);
    put32(&mut active, 51, 1);
    put16(&mut active, 2, (PAGE - length - 8) as u16);
    let mut page = original.to_vec();
    page[..length].copy_from_slice(&active);
    page
}

fn leaf(original: Option<&[u8]>, values: &[[i32; 2]; 3], components: &[(usize, bool)]) -> Vec<u8> {
    let mut entries = values
        .iter()
        .enumerate()
        .map(|(slot, row)| {
            let mut wire = Vec::new();
            for &(column, descending) in components {
                let mut component = vec![0x7f];
                component.extend_from_slice(&((row[column] as u32) ^ 0x8000_0000).to_be_bytes());
                if descending {
                    for byte in &mut component {
                        *byte = !*byte;
                    }
                }
                wire.extend_from_slice(&component);
            }
            wire.extend_from_slice(&((8_u32 << 8) | slot as u32).to_be_bytes());
            wire
        })
        .collect::<Vec<_>>();
    entries.sort();
    let active = entries.iter().map(Vec::len).sum::<usize>();
    assert!(active < 3616);
    let mut page = original.map_or_else(|| vec![0xa7; PAGE], <[u8]>::to_vec);
    page[..480].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    put16(&mut page, 2, (3616 - active) as u16);
    put32(&mut page, 4, 7);
    // Prefix0 deliberately retains fixed single-key geometry and fits composite
    // repacking without ambiguity about unused compression capacity.
    let mut end = 0;
    for wire in entries {
        page[480 + end..480 + end + wire.len()].copy_from_slice(&wire);
        end += wire.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    page
}

/// Derive from qualified canonical BEFORE and explicit literal index facts only.
pub fn source(header: u8, recipe: Recipe) -> Case {
    let program = match recipe {
        Recipe::MacroOnlyId => Program::MacroOnlyTarget,
        Recipe::RestoreOriginal => Program::RestoreOriginal,
        Recipe::Empty => Program::Empty,
        _ => Program::Sequential,
    };
    let original = canonical_before_change::source(header, program);
    let components = match recipe {
        Recipe::Composite => vec![(1, false), (0, true)],
        Recipe::MacroOnlyId => vec![(0, false)],
        _ => vec![(1, recipe == Recipe::Descending)],
    };
    let unique = matches!(
        recipe,
        Recipe::UniqueSwap | Recipe::UniqueConflict | Recipe::MacroOnlyId
    );
    let alias = recipe == Recipe::Alias;
    let mut before = original.before;
    assert_eq!(before.len(), 9 * PAGE);
    before.resize(10 * PAGE, 0);
    let mut maps = records(&before[3 * PAGE..4 * PAGE]);
    assert_eq!(maps.len(), 16);
    assert!(maps.iter().all(|row| row.len() == 13));
    let mut owned = vec![0; 13];
    owned[6] = 2; // Absolute page9, base0, little-endian bitmap bit9.
    maps.push(owned);
    before[3 * PAGE..4 * PAGE].copy_from_slice(&packed_map_rows(&maps));
    let td = definition(&before[7 * PAGE..8 * PAGE], &components, unique, alias);
    before[7 * PAGE..8 * PAGE].copy_from_slice(&td);
    before[9 * PAGE..10 * PAGE].copy_from_slice(&leaf(None, &ROWS, &components));
    let mut updates = original.updates;
    let mut final_rows = ROWS;
    match recipe {
        Recipe::MacroOnlyId => final_rows[0] = [11, 17],
        Recipe::RestoreOriginal | Recipe::Empty => {}
        Recipe::UniqueSwap => {
            updates = vec![
                Update {
                    ordinal: 0,
                    expected: 10,
                    replacement: 9,
                },
                Update {
                    ordinal: 1,
                    expected: 20,
                    replacement: -11,
                },
            ];
            final_rows[0][1] = 20;
            final_rows[1][1] = 10;
        }
        Recipe::UniqueConflict => {
            updates = vec![Update {
                ordinal: 0,
                expected: 10,
                replacement: 9,
            }];
            final_rows[0][1] = 20;
        }
        _ => final_rows[0][1] = 28,
    }
    let changed_rows = final_rows
        .iter()
        .zip(ROWS)
        .filter(|(after, old)| **after != *old)
        .count();
    let expected = (recipe != Recipe::UniqueConflict).then(|| {
        let mut after = before.clone();
        for (slot, row) in final_rows.iter().enumerate() {
            let word = 8 * PAGE + 14 + 2 * slot;
            let start = 8 * PAGE
                + usize::from(u16::from_le_bytes(
                    before[word..word + 2].try_into().unwrap(),
                ));
            assert_eq!(&before[start..start + 2], &[2, 0]);
            assert_eq!(before[start + 10], 3);
            for (column, value) in row.iter().enumerate() {
                let offset = start + 2 + 4 * column;
                assert_eq!(
                    &before[offset..offset + 4],
                    &ROWS[slot][column].to_le_bytes()
                );
                after[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        after[9 * PAGE..10 * PAGE].copy_from_slice(&leaf(
            Some(&before[9 * PAGE..10 * PAGE]),
            &final_rows,
            &components,
        ));
        assert_eq!(&after[..8 * PAGE], &before[..8 * PAGE]);
        assert_eq!(
            &after[9 * PAGE + 480 + 3 * (5 * components.len() + 4)..],
            &before[9 * PAGE + 480 + 3 * (5 * components.len() + 4)..]
        );
        after
    });
    Case {
        before,
        expected,
        updates,
        final_rows,
        changed_rows,
        components,
        unique,
        alias,
        extra: original.extra,
    }
}
