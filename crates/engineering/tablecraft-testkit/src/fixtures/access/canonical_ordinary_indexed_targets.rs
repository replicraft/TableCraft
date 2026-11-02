//! Independent original requests and literal indexed-update outcomes.
use super::canonical_ordinary_indexed::{self as originals, Case as Original};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;

/// Closed acceptance choices. New originals remain read hypotheses until gated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    ManualSeven,
    PromoteNull,
    ClearValue,
    PresentBesideNull,
    UniqueSwap,
    UniqueAllNull,
    Aliases,
    ManualTree,
    PrefixLoss,
    Empty,
    PresentNoop,
    NullNoop,
    UniqueConflict,
    StaleNullExpected,
    GeneratedTarget,
    PrimaryNull,
}

pub const RECIPES: [Recipe; 16] = [
    Recipe::ManualSeven,
    Recipe::PromoteNull,
    Recipe::ClearValue,
    Recipe::PresentBesideNull,
    Recipe::UniqueSwap,
    Recipe::UniqueAllNull,
    Recipe::Aliases,
    Recipe::ManualTree,
    Recipe::PrefixLoss,
    Recipe::Empty,
    Recipe::PresentNoop,
    Recipe::NullNoop,
    Recipe::UniqueConflict,
    Recipe::StaleNullExpected,
    Recipe::GeneratedTarget,
    Recipe::PrimaryNull,
];

impl Recipe {
    pub const fn id(self) -> &'static str {
        match self {
            Self::ManualSeven => "manual-seven",
            Self::PromoteNull => "promote-null",
            Self::ClearValue => "clear-value",
            Self::PresentBesideNull => "present-beside-null",
            Self::UniqueSwap => "unique-swap",
            Self::UniqueAllNull => "unique-all-null",
            Self::Aliases => "aliases",
            Self::ManualTree => "manual-tree",
            Self::PrefixLoss => "prefix-loss",
            Self::Empty => "empty",
            Self::PresentNoop => "present-noop",
            Self::NullNoop => "null-noop",
            Self::UniqueConflict => "unique-conflict",
            Self::StaleNullExpected => "stale-null-expected",
            Self::GeneratedTarget => "generated-target",
            Self::PrimaryNull => "primary-null",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Update {
    pub row: usize,
    pub expected: Option<i32>,
    pub replacement: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    DuplicateKey,
    Conflict,
    GeneratedColumn,
    RequiredIndexKey,
}

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Option<Vec<u8>>,
    pub original_recipe: Original,
    pub derived_original: bool,
    pub rows: Vec<Vec<Value>>,
    pub final_rows: Vec<Vec<Value>>,
    /// One original group: table Items, column Id. Empty means an empty group,
    /// never zero groups or a normalized final request.
    pub updates: Vec<Update>,
    pub changed_rows: usize,
    pub refusal: Option<Refusal>,
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn key(value: Option<i32>, row: usize) -> Vec<u8> {
    let mut key = if let Some(value) = value {
        let mut key = vec![0x7f];
        key.extend_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
        key
    } else {
        vec![0]
    };
    key.extend_from_slice(&((8_u32 << 8) | row as u32).to_be_bytes());
    key
}

fn keys(rows: &[Vec<Value>]) -> Vec<Vec<u8>> {
    let mut keys = rows
        .iter()
        .enumerate()
        .map(|(row, cells)| {
            let value = match cells[0] {
                Value::Null => None,
                Value::Int32(value) => Some(value),
                _ => panic!("original keys use only exact optional Int32"),
            };
            key(value, row)
        })
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn index(original: &[u8], entries: &[Vec<u8>]) -> Vec<u8> {
    assert!(!entries.is_empty());
    assert!(matches!(original[0], 3 | 4));
    let old_prefix = usize::from(u16::from_le_bytes(original[24..26].try_into().unwrap()));
    let mut common = entries[0].len();
    for entry in &entries[1..] {
        common = entries[0][..common]
            .iter()
            .zip(entry)
            .take_while(|(a, b)| a == b)
            .count();
    }
    let prefix = old_prefix.min(common);
    let mut result = original.to_vec();
    result[27..480].fill(0);
    put16(&mut result, 24, prefix as u16);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let suffix = &entry[if position == 0 { 0 } else { prefix }..];
        assert!(end + suffix.len() <= 3616);
        result[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        result[27 + end / 8] |= 1 << (end % 8);
    }
    put16(&mut result, 2, (3616 - end) as u16);
    // Unused/vacated entry bytes, sibling links, owner, levels, stamp and all
    // other original metadata are retained. No allocation or new page exists.
    result
}

fn bounds(bytes: &[u8], row: usize) -> (usize, usize) {
    let base = 8 * PAGE;
    let word = base + 14 + row * 2;
    let start = usize::from(u16::from_le_bytes(
        bytes[word..word + 2].try_into().unwrap(),
    ));
    assert_eq!(start & 0xe000, 0);
    let end = if row == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            bytes[word - 2..word].try_into().unwrap(),
        ))
    };
    assert_eq!(&bytes[base + start..base + start + 2], &[2, 0]);
    (base + start, base + end)
}

fn update(row: usize, expected: Option<i32>, replacement: Option<i32>) -> Update {
    Update {
        row,
        expected,
        replacement,
    }
}

/// Reuses ORIGINAL bytes only. No previous AFTER or planner output is consumed.
pub fn source(header: u8, recipe: Recipe) -> Case {
    assert!(matches!(header, 5 | 6));
    let original_recipe = match recipe {
        Recipe::PromoteNull
        | Recipe::ClearValue
        | Recipe::PresentBesideNull
        | Recipe::NullNoop
        | Recipe::StaleNullExpected => Original::NullableOrdinary,
        Recipe::Aliases | Recipe::PrefixLoss => Original::AliasHistory,
        Recipe::ManualTree | Recipe::PrimaryNull => Original::RetainedTree,
        Recipe::GeneratedTarget => Original::CounterPrimary,
        _ => Original::UniqueManual,
    };
    let original = originals::source(original_recipe);
    assert_eq!(original.columns, ["Id", "Score"]);
    let mut before = original.bytes;
    before[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
    let rows = original.rows;
    let derived_original = matches!(
        recipe,
        Recipe::ManualTree | Recipe::PrimaryNull | Recipe::PrefixLoss
    );
    if matches!(recipe, Recipe::ManualTree | Recipe::PrimaryNull) {
        assert_eq!(before[7 * PAGE + 75 + 15], 7);
        before[7 * PAGE + 20..7 * PAGE + 24].fill(0);
        before[7 * PAGE + 75 + 15] = 3;
    }
    if recipe == Recipe::PrefixLoss {
        let mut page = before[9 * PAGE..10 * PAGE].to_vec();
        put16(&mut page, 24, 4);
        before[9 * PAGE..10 * PAGE].copy_from_slice(&index(&page, &keys(&rows)));
        assert_eq!(
            u16::from_le_bytes(before[9 * PAGE + 2..9 * PAGE + 4].try_into().unwrap()),
            3602
        );
    }

    let (updates, refusal) = match recipe {
        Recipe::ManualSeven | Recipe::Aliases | Recipe::ManualTree => {
            (vec![update(0, Some(1), Some(7))], None)
        }
        Recipe::PromoteNull => (vec![update(1, None, Some(2))], None),
        Recipe::ClearValue => (vec![update(0, Some(1), None)], None),
        Recipe::PresentBesideNull => (vec![update(0, Some(1), Some(7))], None),
        Recipe::UniqueSwap => (
            vec![update(0, Some(1), Some(2)), update(1, Some(2), Some(1))],
            None,
        ),
        Recipe::UniqueAllNull => (
            vec![update(0, Some(1), None), update(1, Some(2), None)],
            None,
        ),
        Recipe::PrefixLoss => (vec![update(0, Some(1), Some(i32::MIN))], None),
        Recipe::Empty => (Vec::new(), None),
        Recipe::PresentNoop => (vec![update(0, Some(1), Some(1))], None),
        Recipe::NullNoop => (vec![update(1, None, None)], None),
        Recipe::UniqueConflict => (
            vec![update(0, Some(1), Some(2))],
            Some(Refusal::DuplicateKey),
        ),
        Recipe::StaleNullExpected => (vec![update(1, Some(2), Some(7))], Some(Refusal::Conflict)),
        Recipe::GeneratedTarget => (
            vec![update(0, Some(1), Some(7))],
            Some(Refusal::GeneratedColumn),
        ),
        Recipe::PrimaryNull => (
            vec![update(0, Some(1), None)],
            Some(Refusal::RequiredIndexKey),
        ),
    };
    let mut final_rows = rows.clone();
    let mut after = before.clone();
    let mut changed_rows = 0;
    if refusal.is_none() {
        for request in &updates {
            let (start, end) = bounds(&before, request.row);
            let present = before[end - 1] & 1 != 0;
            assert_eq!(present, request.expected.is_some());
            if let Some(value) = request.expected {
                assert_eq!(&before[start + 2..start + 6], &value.to_le_bytes());
            }
            if request.expected == request.replacement {
                continue;
            }
            changed_rows += 1;
            if let Some(value) = request.replacement {
                after[start + 2..start + 6].copy_from_slice(&value.to_le_bytes());
                after[end - 1] |= 1;
                final_rows[request.row][0] = Value::Int32(value);
            } else {
                after[end - 1] &= !1;
                final_rows[request.row][0] = Value::Null;
            }
        }
        if changed_rows != 0 {
            let entries = keys(&final_rows);
            if recipe == Recipe::ManualTree {
                let mut bound = entries[0].clone();
                bound.extend_from_slice(&10_u32.to_be_bytes());
                after[9 * PAGE..10 * PAGE]
                    .copy_from_slice(&index(&before[9 * PAGE..10 * PAGE], &[bound]));
                after[10 * PAGE..11 * PAGE]
                    .copy_from_slice(&index(&before[10 * PAGE..11 * PAGE], &entries[..1]));
                after[11 * PAGE..12 * PAGE]
                    .copy_from_slice(&index(&before[11 * PAGE..12 * PAGE], &entries[1..]));
            } else {
                after[9 * PAGE..10 * PAGE]
                    .copy_from_slice(&index(&before[9 * PAGE..10 * PAGE], &entries));
            }
        }
    }
    Case {
        before,
        expected: refusal.is_none().then_some(after),
        original_recipe,
        derived_original,
        rows,
        final_rows,
        updates,
        changed_rows,
        refusal,
    }
}
