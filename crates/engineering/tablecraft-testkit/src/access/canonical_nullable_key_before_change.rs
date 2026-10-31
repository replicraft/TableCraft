//! Independent untouched-NULL key sources; no writer/encoder imports.
use super::{
    canonical_indexed_mixed_before_change as indexed, canonical_mixed_before_change::Update,
};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;

/// Closed authored hypotheses, to be read-qualified before any producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    /// One untouched NULL sorts first in an ascending scalar leaf.
    ScalarAscending,
    /// One untouched NULL sorts last in a descending scalar leaf.
    ScalarDescending,
    /// Two stored all-NULL keys have no unique duplicate identity.
    UniqueNullPair,
    /// A partial-NULL tuple remains a real key with readonly Id.
    CompositePartialNull,
    /// The left retained leaf contains NULL plus one present key.
    TreeNullFirst,
    /// A NULL node bound retains its logical ROW and child suffixes.
    TreeNullBound,
}

/// Stable source gate/export order.
pub const RECIPES: [Recipe; 6] = [
    Recipe::ScalarAscending,
    Recipe::ScalarDescending,
    Recipe::UniqueNullPair,
    Recipe::CompositePartialNull,
    Recipe::TreeNullFirst,
    Recipe::TreeNullBound,
];

impl Recipe {
    /// Artifact identity, not inferred from reader output.
    pub const fn id(self) -> &'static str {
        match self {
            Self::ScalarAscending => "scalar-ascending",
            Self::ScalarDescending => "scalar-descending",
            Self::UniqueNullPair => "unique-null-pair",
            Self::CompositePartialNull => "composite-partial-null",
            Self::TreeNullFirst => "tree-null-first",
            Self::TreeNullBound => "tree-null-bound",
        }
    }

    /// Only committed BEFORE/rows/Extra from this original owner are consumed.
    pub const fn seed(self) -> indexed::Recipe {
        match self {
            Self::UniqueNullPair => indexed::Recipe::CounterUniqueSwap,
            Self::CompositePartialNull => indexed::Recipe::CounterCompositeTie,
            Self::TreeNullFirst | Self::TreeNullBound => indexed::Recipe::CounterTreePrefixLoss,
            _ => indexed::Recipe::CounterLeafPrefixLoss,
        }
    }
}

/// Complete original and independently constructed intended result.
pub struct Case {
    /// Eleven-page leaf or thirteen-page node original.
    pub before: Vec<u8>,
    /// Complete literal intended result, never obtained from a producer.
    pub expected: Vec<u8>,
    /// Original complete scalar values, with actual NULL Score cells.
    pub rows: Vec<Vec<Value>>,
    /// Literal complete final scalar rows.
    pub final_rows: Vec<Vec<Value>>,
    /// Exactly one originally present-I32 Score request.
    pub update: Update,
    /// Native field number/direction in selected key order.
    pub components: Vec<(usize, bool)>,
    /// Selected declared unique rule; never an ignore-NULL flag.
    pub unique: bool,
    /// Original NULL Score row ordinals; inactive bytes are retained.
    pub null_rows: Vec<usize>,
    /// Root10 has two retained leaves instead of being a leaf.
    pub node: bool,
    /// Left leaf population, preserving the original partition.
    pub left_count: usize,
    /// Selected index's original name.
    pub index_name: &'static str,
    /// Explicit selected owned pages; PrimaryId9 is outside this set.
    pub owned: Vec<u32>,
    /// Native-owned original MR2 event payload.
    pub extra: Vec<u8>,
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn location(bytes: &[u8], ordinal: usize) -> (usize, usize) {
    let page = 8 * PAGE;
    assert_eq!(&bytes[page + 12..page + 14], &3_u16.to_le_bytes());
    let word = page + 14 + 2 * ordinal;
    let start = u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap());
    assert_eq!(start & 0xe000, 0);
    let end = if ordinal == 0 {
        9 * PAGE
    } else {
        page + usize::from(u16::from_le_bytes(
            bytes[word - 2..word].try_into().unwrap(),
        ))
    };
    (page + usize::from(start), end)
}

fn score_physical(td: &[u8]) -> usize {
    assert_eq!(&td[45..47], &6_u16.to_le_bytes());
    assert_eq!(&td[47..51], &2_u32.to_le_bytes());
    assert_eq!(&td[51..55], &2_u32.to_le_bytes());
    let mut names_end = 63 + 2 * 12 + 6 * 25;
    for _ in 0..6 {
        let width = usize::from(u16::from_le_bytes(
            td[names_end..names_end + 2].try_into().unwrap(),
        ));
        names_end += 2 + width;
    }
    names_end + 52
}

fn key(score: Option<i32>, ordinal: usize, components: &[(usize, bool)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &(number, descending) in components {
        let value = match number {
            0 => Some(ordinal as i32 + 1),
            5 => score,
            _ => panic!("literal I32 key"),
        };
        let mut component = match value {
            None => vec![0],
            Some(value) => {
                let mut component = vec![0x7f];
                component.extend_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
                component
            }
        };
        if descending {
            for byte in &mut component {
                *byte = !*byte;
            }
        }
        bytes.extend_from_slice(&component);
    }
    bytes.extend_from_slice(&((8_u32 << 8) | ordinal as u32).to_be_bytes());
    bytes
}

fn keys(scores: &[Option<i32>; 3], components: &[(usize, bool)]) -> Vec<Vec<u8>> {
    let mut result = scores
        .iter()
        .enumerate()
        .map(|(ordinal, value)| key(*value, ordinal, components))
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn page(original: Option<&[u8]>, entries: &[Vec<u8>], links: [u32; 3], node: bool) -> Vec<u8> {
    assert!(!entries.is_empty());
    let active = entries.iter().map(Vec::len).sum::<usize>();
    assert!(active < 3616);
    let mut bytes = original.map_or_else(|| vec![0xa7; PAGE], <[u8]>::to_vec);
    bytes[..480].fill(0);
    bytes[..2].copy_from_slice(&[if node { 3 } else { 4 }, 1]);
    put16(&mut bytes, 2, (3616 - active) as u16);
    put32(&mut bytes, 4, 7);
    put32(&mut bytes, 12, links[0]);
    put32(&mut bytes, 16, links[1]);
    put32(&mut bytes, 20, links[2]);
    // Prefix0 is an explicit source choice for variable-width components.
    put16(&mut bytes, 24, 0);
    bytes[26] = u8::from(node);
    let mut end = 0;
    for entry in entries {
        bytes[480 + end..480 + end + entry.len()].copy_from_slice(entry);
        end += entry.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    bytes
}

fn install(image: &mut [u8], number: usize, page: &[u8]) {
    image[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn index(image: &mut [u8], original: Option<&[u8]>, case: &Case, scores: &[Option<i32>; 3]) {
    let entries = keys(scores, &case.components);
    let old = |number: usize| original.map(|bytes| &bytes[number * PAGE..(number + 1) * PAGE]);
    if case.node {
        let left = &entries[..case.left_count];
        let right = &entries[case.left_count..];
        let mut bound = left.last().unwrap().clone();
        bound.extend_from_slice(&11_u32.to_be_bytes());
        install(image, 10, &page(old(10), &[bound], [0, 0, 12], true));
        install(image, 11, &page(old(11), left, [0, 12, 0], false));
        install(image, 12, &page(old(12), right, [11, 0, 0], false));
    } else {
        install(image, 10, &page(old(10), &entries, [0, 0, 0], false));
    }
}

/// Author from committed original bytes and explicit NULL mask/row decisions.
/// UniqueNullPair is a read-qualification hypothesis, not native Access evidence.
pub fn source(header: u8, recipe: Recipe) -> Case {
    let seed = indexed::source(header, recipe.seed());
    let node = matches!(recipe, Recipe::TreeNullFirst | Recipe::TreeNullBound);
    let null_rows = if recipe == Recipe::UniqueNullPair {
        vec![1, 2]
    } else {
        vec![2]
    };
    let replacement = match recipe {
        Recipe::UniqueNullPair => 17,
        Recipe::CompositePartialNull => 999,
        _ => 2599,
    };
    let final_score: i32 = match recipe {
        Recipe::UniqueNullPair => 1018,
        Recipe::CompositePartialNull => 2000,
        _ => 3600,
    };
    let mut case = Case {
        before: seed.before,
        expected: Vec::new(),
        rows: seed.rows,
        final_rows: Vec::new(),
        update: Update {
            ordinal: 0,
            expected: 1000,
            replacement,
        },
        components: if recipe == Recipe::ScalarDescending {
            vec![(5, true)]
        } else {
            seed.components
        },
        unique: seed.unique,
        null_rows,
        node,
        left_count: if recipe == Recipe::TreeNullBound {
            1
        } else {
            2
        },
        index_name: seed.index_name,
        owned: seed.owned,
        extra: seed.extra,
    };
    for &ordinal in &case.null_rows {
        let (start, end) = location(&case.before, ordinal);
        assert_eq!(&case.before[start..start + 2], &6_u16.to_le_bytes());
        assert_eq!(
            &case.before[start + 6..start + 10],
            &((ordinal as i32 + 1) * 1000).to_le_bytes()
        );
        assert_eq!(case.before[end - 1] & 0x20, 0x20);
        case.before[end - 1] &= !0x20;
        case.rows[ordinal][5] = Value::Null;
    }
    if recipe == Recipe::ScalarDescending {
        let physical = score_physical(&case.before[7 * PAGE..8 * PAGE]);
        let direction = 7 * PAGE + physical + 6;
        assert_eq!(&case.before[direction - 2..direction], &5_u16.to_le_bytes());
        assert_eq!(case.before[direction], 1);
        case.before[direction] = 0;
    }
    let mut original_scores = [Some(1000), Some(2000), Some(3000)];
    for &ordinal in &case.null_rows {
        original_scores[ordinal] = None;
    }
    let mut before = case.before.clone();
    index(&mut before, None, &case, &original_scores);
    case.before = before;
    let mut after = case.before.clone();
    let (start, _) = location(&case.before, 0);
    assert_eq!(&case.before[start + 6..start + 10], &1000_i32.to_le_bytes());
    after[start + 6..start + 10].copy_from_slice(&final_score.to_le_bytes());
    let mut final_scores = original_scores;
    final_scores[0] = Some(final_score);
    index(&mut after, Some(&case.before), &case, &final_scores);
    assert_eq!(&after[..8 * PAGE], &case.before[..8 * PAGE]);
    assert_eq!(
        &after[9 * PAGE..10 * PAGE],
        &case.before[9 * PAGE..10 * PAGE]
    );
    assert_eq!(
        &after[8 * PAGE..start + 6],
        &case.before[8 * PAGE..start + 6]
    );
    assert_eq!(
        &after[start + 10..9 * PAGE],
        &case.before[start + 10..9 * PAGE]
    );
    for &ordinal in &case.null_rows {
        let (start, end) = location(&case.before, ordinal);
        assert_eq!(&after[start..end], &case.before[start..end]);
    }
    if recipe == Recipe::TreeNullBound {
        assert_eq!(
            &after[10 * PAGE..12 * PAGE],
            &case.before[10 * PAGE..12 * PAGE]
        );
    }
    case.final_rows = case.rows.clone();
    case.final_rows[0][5] = Value::Int32(final_score);
    case.expected = after;
    case
}
