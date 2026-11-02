//! Independent retained-node/compressed-index SOURCE and complete expectations.
//! Only qualified BEFORE owners are reused; no production writer is imported.
use super::{
    canonical_before_change::{self, Recipe as Program, Update},
    canonical_indexed_before_change::{self as indexed, Recipe as Seed},
};

const PAGE: usize = 4096;
const ORIGINAL: [[i32; 2]; 3] = [[1, 10], [2, 20], [3, 30]];

/// Finite intended-positive profiles, qualified before any producer execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    /// One leaf with an actual shared four-byte prefix.
    LeafPrefix,
    /// Root with a tail child and two uncompressed leaves.
    NodeAscending,
    /// Descending keys with shared prefixes on root and leaves.
    NodeDescendingPrefix,
    /// Composite keys include a macro-only changed Id component.
    NodeCompositeMacro,
    /// Unique swap crosses the retained leaf partition.
    NodeUniqueSwap,
    /// Single leaf loses its old common prefix but fits after repacking.
    LeafPrefixLoss,
    /// Left child loses its prefix while parent topology remains unchanged.
    NodePrefixLoss,
    /// Both children have explicit upper bounds; header tail is zero.
    NodeZeroTail,
    /// A valid loose upper bound remains exact for a whole-index no-op.
    NodeRetainedUpper,
}

/// Qualification order, separate from production routing or observations.
pub const RECIPES: [Recipe; 9] = [
    Recipe::LeafPrefix,
    Recipe::NodeAscending,
    Recipe::NodeDescendingPrefix,
    Recipe::NodeCompositeMacro,
    Recipe::NodeUniqueSwap,
    Recipe::LeafPrefixLoss,
    Recipe::NodePrefixLoss,
    Recipe::NodeZeroTail,
    Recipe::NodeRetainedUpper,
];

impl Recipe {
    /// Stable artifact identifier.
    pub const fn id(self) -> &'static str {
        match self {
            Self::LeafPrefix => "leaf-prefix",
            Self::NodeAscending => "node-ascending",
            Self::NodeDescendingPrefix => "node-descending-prefix",
            Self::NodeCompositeMacro => "node-composite-macro",
            Self::NodeUniqueSwap => "node-unique-swap",
            Self::LeafPrefixLoss => "leaf-prefix-loss",
            Self::NodePrefixLoss => "node-prefix-loss",
            Self::NodeZeroTail => "node-zero-tail",
            Self::NodeRetainedUpper => "node-retained-upper",
        }
    }
}

/// Exact authored source and independently encoded final image.
pub struct Case {
    /// Complete original; 10 pages for leaves, 12 for node profiles.
    pub before: Vec<u8>,
    /// Complete intended positive final image, never learned from a writer.
    pub expected: Vec<u8>,
    /// Exact original Score requests, including real RestoreOriginal intent.
    pub updates: Vec<Update>,
    /// Literal final scalar rows in original source order.
    pub final_rows: [[i32; 2]; 3],
    /// Distinct original roots with final byte changes.
    pub changed_rows: usize,
    /// Native key number/direction in tuple order.
    pub components: Vec<(usize, bool)>,
    /// Physical unique declaration.
    pub unique: bool,
    /// Exact catalog-owned MR2 payload.
    pub extra: Vec<u8>,
    /// Whether root9 is a node with two leaf children.
    pub node: bool,
    /// Whether both children have explicit bound entries instead of a tail.
    pub zero_tail: bool,
    /// Whether the original loose bound25 is retained for unchanged final keys.
    pub retained_upper: bool,
    /// Explicit owned page set, independent of a producer allocation plan.
    pub owned: Vec<u32>,
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn key(row: [i32; 2], slot: u32, components: &[(usize, bool)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &(column, descending) in components {
        let mut component = vec![0x7f];
        component.extend_from_slice(&((row[column] as u32) ^ 0x8000_0000).to_be_bytes());
        if descending {
            for byte in &mut component {
                *byte = !*byte;
            }
        }
        bytes.extend_from_slice(&component);
    }
    bytes.extend_from_slice(&((8_u32 << 8) | slot).to_be_bytes());
    bytes
}

fn keys(rows: &[[i32; 2]; 3], components: &[(usize, bool)]) -> Vec<Vec<u8>> {
    let mut keys = rows
        .iter()
        .enumerate()
        .map(|(slot, row)| key(*row, slot as u32, components))
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn page(
    original: Option<&[u8]>,
    entries: &[Vec<u8>],
    prefix_seed: usize,
    links: [u32; 3],
    node: bool,
) -> Vec<u8> {
    assert!(!entries.is_empty());
    let mut common = entries[0].len();
    for entry in &entries[1..] {
        common = entries[0][..common]
            .iter()
            .zip(entry)
            .take_while(|(a, b)| a == b)
            .count();
    }
    let prefix = prefix_seed.min(common);
    // No capacity-pressure policy is needed for these three-row fixtures.
    // Prefix loss is a positive fitting outcome, not an encoder-error retry.
    let active = entries.iter().map(Vec::len).sum::<usize>() - (entries.len() - 1) * prefix;
    assert!(active <= 3616);
    let mut bytes = original.map_or_else(|| vec![0xa7; PAGE], <[u8]>::to_vec);
    bytes[..480].fill(0);
    bytes[..2].copy_from_slice(&[if node { 3 } else { 4 }, 1]);
    put16(&mut bytes, 2, (3616 - active) as u16);
    put32(&mut bytes, 4, 7);
    put32(&mut bytes, 12, links[0]);
    put32(&mut bytes, 16, links[1]);
    put32(&mut bytes, 20, links[2]);
    put16(&mut bytes, 24, prefix as u16);
    bytes[26] = u8::from(node);
    let mut end = 0;
    for (ordinal, entry) in entries.iter().enumerate() {
        let shared = if ordinal == 0 { 0 } else { prefix };
        let suffix = &entry[shared..];
        bytes[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    bytes
}

fn node_entries(
    keys: &[Vec<u8>],
    zero_tail: bool,
    loose: bool,
    components: &[(usize, bool)],
) -> Vec<Vec<u8>> {
    let mut left = if loose {
        key([2, 25], 1, components)
    } else {
        keys[1].clone()
    };
    left.extend_from_slice(&10_u32.to_be_bytes());
    let mut entries = vec![left];
    if zero_tail {
        let mut right = keys[2].clone();
        right.extend_from_slice(&11_u32.to_be_bytes());
        entries.push(right);
    }
    entries
}

fn install(image: &mut [u8], number: usize, page: &[u8]) {
    image[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

/// Construct from only pinned original fixtures, literal intents and raw framing.
pub fn source(header: u8, recipe: Recipe) -> Case {
    let seed = match recipe {
        Recipe::NodeDescendingPrefix => Seed::Descending,
        Recipe::NodeCompositeMacro => Seed::Composite,
        Recipe::NodeUniqueSwap => Seed::UniqueSwap,
        Recipe::NodeRetainedUpper => Seed::RestoreOriginal,
        _ => Seed::Ascending,
    };
    let old = indexed::source(header, seed);
    let node = !matches!(recipe, Recipe::LeafPrefix | Recipe::LeafPrefixLoss);
    let zero_tail = recipe == Recipe::NodeZeroTail;
    let retained_upper = recipe == Recipe::NodeRetainedUpper;
    let prefix = if recipe == Recipe::NodeAscending {
        0
    } else {
        4
    };
    let components = old.components;
    let unique = old.unique;
    let mut before = old.before;
    let mut extra = old.extra;
    let mut updates = old.updates;
    let mut final_rows = ORIGINAL;
    match recipe {
        Recipe::NodeCompositeMacro => {
            let macro_only = canonical_before_change::source(header, Program::MacroOnlyTarget);
            before[4 * PAGE..5 * PAGE].copy_from_slice(&macro_only.before[4 * PAGE..5 * PAGE]);
            extra = macro_only.extra;
            updates = macro_only.updates;
            final_rows[0] = [11, 17];
        }
        Recipe::NodeUniqueSwap => {
            updates = vec![
                Update {
                    ordinal: 1,
                    expected: 20,
                    replacement: 9,
                },
                Update {
                    ordinal: 2,
                    expected: 30,
                    replacement: -11,
                },
            ];
            final_rows[1][1] = 30;
            final_rows[2][1] = 20;
        }
        Recipe::LeafPrefixLoss => {
            updates = vec![Update {
                ordinal: 0,
                expected: 10,
                replacement: 2_147_483_636,
            }];
            final_rows[0][1] = 2_147_483_647;
        }
        Recipe::NodePrefixLoss => {
            updates = vec![Update {
                ordinal: 2,
                expected: 30,
                replacement: -51,
            }];
            final_rows[2][1] = -20;
        }
        Recipe::NodeRetainedUpper => {}
        _ => final_rows[0][1] = 28,
    }
    let original_keys = keys(&ORIGINAL, &components);
    if node {
        before.resize(12 * PAGE, 0);
        let directory = 3 * PAGE + 14 + 2 * 16;
        let map = 3 * PAGE
            + usize::from(u16::from_le_bytes(
                before[directory..directory + 2].try_into().unwrap(),
            ));
        assert_eq!(
            &before[map..map + 13],
            &[0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0]
        );
        before[map + 6] = 14;
        install(
            &mut before,
            10,
            &page(None, &original_keys[..2], prefix, [0, 11, 0], false),
        );
        install(
            &mut before,
            11,
            &page(None, &original_keys[2..], prefix, [10, 0, 0], false),
        );
        let entries = node_entries(&original_keys, zero_tail, retained_upper, &components);
        install(
            &mut before,
            9,
            &page(
                None,
                &entries,
                prefix,
                [0, 0, if zero_tail { 0 } else { 11 }],
                true,
            ),
        );
    } else {
        install(
            &mut before,
            9,
            &page(None, &original_keys, prefix, [0, 0, 0], false),
        );
    }
    let changed_rows = final_rows
        .iter()
        .zip(ORIGINAL)
        .filter(|(new, old)| **new != *old)
        .count();
    let mut expected = before.clone();
    for (slot, row) in final_rows.iter().enumerate() {
        let word = 8 * PAGE + 14 + 2 * slot;
        let start = 8 * PAGE
            + usize::from(u16::from_le_bytes(
                before[word..word + 2].try_into().unwrap(),
            ));
        assert_eq!(&before[start..start + 2], &[2, 0]);
        assert_eq!(before[start + 10], 3);
        for (column, value) in row.iter().enumerate() {
            let at = start + 2 + 4 * column;
            assert_eq!(&before[at..at + 4], &ORIGINAL[slot][column].to_le_bytes());
            expected[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    if changed_rows != 0 {
        let final_keys = keys(&final_rows, &components);
        if node {
            let left = page(
                Some(&before[10 * PAGE..11 * PAGE]),
                &final_keys[..2],
                prefix,
                [0, 11, 0],
                false,
            );
            let right = page(
                Some(&before[11 * PAGE..12 * PAGE]),
                &final_keys[2..],
                prefix,
                [10, 0, 0],
                false,
            );
            let entries = node_entries(&final_keys, zero_tail, false, &components);
            let root = page(
                Some(&before[9 * PAGE..10 * PAGE]),
                &entries,
                prefix,
                [0, 0, if zero_tail { 0 } else { 11 }],
                true,
            );
            install(&mut expected, 10, &left);
            install(&mut expected, 11, &right);
            install(&mut expected, 9, &root);
        } else {
            let leaf = page(
                Some(&before[9 * PAGE..10 * PAGE]),
                &final_keys,
                prefix,
                [0, 0, 0],
                false,
            );
            install(&mut expected, 9, &leaf);
        }
    }
    assert_eq!(&expected[..8 * PAGE], &before[..8 * PAGE]);
    if retained_upper {
        assert_eq!(expected, before);
    }
    Case {
        before,
        expected,
        updates,
        final_rows,
        changed_rows,
        components,
        unique,
        extra,
        node,
        zero_tail,
        retained_upper,
        owned: if node { vec![9, 10, 11] } else { vec![9] },
    }
}
