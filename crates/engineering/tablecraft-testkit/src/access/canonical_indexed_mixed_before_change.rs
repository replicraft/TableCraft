//! Original-only mixed scalar INDEX sources and literal BeforeChange outcomes.
use super::canonical_mixed_before_change::{self as mixed, Update};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const SCORES: [i32; 3] = [1000, 2000, 3000];

/// Finite source shapes, authored before the indexed mixed producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    /// Manual Id, nonunique ascending Score reorder.
    ManualAscending,
    /// Atomic unique Score swap preserves the separate counter index.
    CounterUniqueSwap,
    /// Healthy unique original; final duplicate has no expected image.
    CounterUniqueConflict,
    /// Equal Scores remain unique through descending readonly Id components.
    CounterCompositeTie,
    /// Positive fitting prefix loss on a single leaf.
    CounterLeafPrefixLoss,
    /// Repartition into retained leaves, with left prefix loss.
    CounterTreePrefixLoss,
    /// Exact restore retains a valid loose interior upper bound.
    CounterTreeRetainedRestore,
}

/// Stable source qualification order.
pub const RECIPES: [Recipe; 7] = [
    Recipe::ManualAscending,
    Recipe::CounterUniqueSwap,
    Recipe::CounterUniqueConflict,
    Recipe::CounterCompositeTie,
    Recipe::CounterLeafPrefixLoss,
    Recipe::CounterTreePrefixLoss,
    Recipe::CounterTreeRetainedRestore,
];

impl Recipe {
    /// Artifact identity, independent of observations.
    pub const fn id(self) -> &'static str {
        match self {
            Self::ManualAscending => "manual-ascending",
            Self::CounterUniqueSwap => "counter-unique-swap",
            Self::CounterUniqueConflict => "counter-unique-conflict",
            Self::CounterCompositeTie => "counter-composite-tie",
            Self::CounterLeafPrefixLoss => "counter-leaf-prefix-loss",
            Self::CounterTreePrefixLoss => "counter-tree-prefix-loss",
            Self::CounterTreeRetainedRestore => "counter-tree-retained-restore",
        }
    }

    /// Only the original bytes/rows/program of this committed source are used.
    pub const fn seed(self) -> mixed::Recipe {
        match self {
            Self::ManualAscending => mixed::Recipe::VariableManual,
            Self::CounterTreeRetainedRestore => mixed::Recipe::CounterRestore,
            _ => mixed::Recipe::CounterPrimary,
        }
    }
}

/// Complete original and independently constructed intended result.
pub struct Case {
    /// Complete new original, with explicit physical index roles.
    pub before: Vec<u8>,
    /// Absent only for the intended final unique conflict.
    pub expected: Option<Vec<u8>>,
    /// All original scalar cells, including unchanged variable carriers.
    pub rows: Vec<Vec<Value>>,
    /// Literal complete final rows; absent for the conflict.
    pub final_rows: Option<Vec<Vec<Value>>>,
    /// Written Score intents, never macro-final values.
    pub updates: Vec<Update>,
    /// Final distinct changed roots; zero for restore/conflict.
    pub changed_rows: usize,
    /// Actual source retains a separate readonly primary counter index.
    pub primary: bool,
    /// New Score index name.
    pub index_name: &'static str,
    /// New physical/logical index number.
    pub data_number: u32,
    /// Explicit new index root page.
    pub root: u32,
    /// Explicit new owned-map record reference.
    pub usage_map: u32,
    /// Complete new index page set.
    pub owned: Vec<u32>,
    /// Native component number and descending direction.
    pub components: Vec<(usize, bool)>,
    /// New index physically declares uniqueness.
    pub unique: bool,
    /// Root has one explicit bound and a separate right tail.
    pub node: bool,
    /// Whole final image retains the original loose upper bound.
    pub retained_upper: bool,
    /// Exact existing catalog-owned event payload.
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

fn append_map(original: &[u8], members: &[u32]) -> Vec<u8> {
    let mut rows = records(original);
    let mut new = vec![0; 13];
    for &member in members {
        assert!(member < 64);
        new[5 + member as usize / 8] |= 1 << (member % 8);
    }
    rows.push(new);
    let mut page = vec![0; PAGE];
    page[..14].copy_from_slice(&original[..14]);
    put16(&mut page, 12, rows.len() as u16);
    let mut end = PAGE;
    for (slot, row) in rows.iter().enumerate() {
        end -= row.len();
        page[end..end + row.len()].copy_from_slice(row);
        put16(&mut page, 14 + 2 * slot, end as u16);
    }
    put16(&mut page, 2, (end - 14 - 2 * rows.len()) as u16);
    assert_eq!(&records(&page)[..rows.len() - 1], &records(original));
    page
}

fn definition(original: &[u8], case: &Case) -> Vec<u8> {
    let old_count = usize::from(case.primary);
    assert_eq!(&original[45..47], &6_u16.to_le_bytes());
    assert_eq!(&original[47..51], &(old_count as u32).to_le_bytes());
    assert_eq!(&original[51..55], &(old_count as u32).to_le_bytes());
    let length = u32::from_le_bytes(original[8..12].try_into().unwrap()) as usize;
    let columns = 63 + old_count * 12;
    let mut names_end = columns + 6 * 25;
    for _ in 0..6 {
        let width = usize::from(u16::from_le_bytes(
            original[names_end..names_end + 2].try_into().unwrap(),
        ));
        names_end += 2 + width;
    }
    let logical_start = names_end + old_count * 52;
    let index_names = logical_start + old_count * 28;
    assert_eq!(&original[length - 2..length], &[0xff, 0xff]);
    let mut active = original[..columns].to_vec();
    let mut counter = [0; 12];
    put32(&mut counter, 4, 3);
    active.extend_from_slice(&counter);
    active.extend_from_slice(&original[columns..names_end]);
    active.extend_from_slice(&original[names_end..logical_start]);
    let mut physical = [0; 52];
    put32(&mut physical, 0, 1923);
    for position in 0..10 {
        put16(&mut physical, 4 + 3 * position, 0xffff);
    }
    for (position, &(number, descending)) in case.components.iter().enumerate() {
        put16(&mut physical, 4 + 3 * position, number as u16);
        physical[6 + 3 * position] = u8::from(!descending);
    }
    put32(&mut physical, 34, case.usage_map);
    put32(&mut physical, 38, case.root);
    physical[46] = if case.unique { 0x81 } else { 0x80 };
    active.extend_from_slice(&physical);
    active.extend_from_slice(&original[logical_start..index_names]);
    let mut logical = [0; 28];
    put32(&mut logical, 0, 1625);
    put32(&mut logical, 4, case.data_number);
    put32(&mut logical, 8, case.data_number);
    put32(&mut logical, 13, u32::MAX);
    active.extend_from_slice(&logical);
    active.extend_from_slice(&original[index_names..length - 2]);
    let name = case
        .index_name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    active.extend_from_slice(&(name.len() as u16).to_le_bytes());
    active.extend_from_slice(&name);
    active.extend_from_slice(&[0xff, 0xff]);
    let size = active.len();
    put32(&mut active, 8, size as u32);
    put32(&mut active, 47, old_count as u32 + 1);
    put32(&mut active, 51, old_count as u32 + 1);
    put16(&mut active, 2, (PAGE - size - 8) as u16);
    let mut page = vec![0; PAGE];
    page[..size].copy_from_slice(&active);
    let new_columns = columns + 12;
    assert_eq!(
        &page[63..columns],
        &original[63..columns],
        "existing physical population counters"
    );
    assert_eq!(
        &page[new_columns..names_end + 12],
        &original[columns..names_end],
        "all original field descriptors and names"
    );
    assert_eq!(
        &page[names_end + 12..logical_start + 12],
        &original[names_end..logical_start],
        "original PrimaryId physical declaration"
    );
    assert_eq!(
        &page[logical_start + 64..index_names + 64],
        &original[logical_start..index_names],
        "original PrimaryId logical declaration"
    );
    assert_eq!(
        &page[index_names + 92..index_names + 92 + length - 2 - index_names],
        &original[index_names..length - 2],
        "original PrimaryId spelling"
    );
    page
}

fn key(score: i32, id: i32, ordinal: u32, components: &[(usize, bool)]) -> Vec<u8> {
    let mut wire = Vec::new();
    for &(number, descending) in components {
        let value = match number {
            0 => id,
            5 => score,
            _ => panic!("literal I32 component"),
        };
        let mut part = vec![0x7f];
        part.extend_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
        if descending {
            for byte in &mut part {
                *byte = !*byte;
            }
        }
        wire.extend_from_slice(&part);
    }
    wire.extend_from_slice(&((8_u32 << 8) | ordinal).to_be_bytes());
    wire
}

fn keys(scores: &[i32; 3], components: &[(usize, bool)]) -> Vec<Vec<u8>> {
    let mut result = scores
        .iter()
        .enumerate()
        .map(|(slot, &score)| key(score, slot as i32 + 1, slot as u32, components))
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn index_page(
    original: Option<&[u8]>,
    entries: &[Vec<u8>],
    links: [u32; 3],
    node: bool,
) -> Vec<u8> {
    let mut common = entries[0].len();
    for entry in &entries[1..] {
        common = entries[0][..common]
            .iter()
            .zip(entry)
            .take_while(|(a, b)| a == b)
            .count();
    }
    let prefix = 3.min(common);
    let active = entries.iter().map(Vec::len).sum::<usize>() - (entries.len() - 1) * prefix;
    assert!(active < 3616);
    let mut page = original.map_or_else(|| vec![0xa7; PAGE], <[u8]>::to_vec);
    page[..480].fill(0);
    page[..2].copy_from_slice(&[if node { 3 } else { 4 }, 1]);
    put16(&mut page, 2, (3616 - active) as u16);
    put32(&mut page, 4, 7);
    put32(&mut page, 12, links[0]);
    put32(&mut page, 16, links[1]);
    put32(&mut page, 20, links[2]);
    put16(&mut page, 24, prefix as u16);
    page[26] = u8::from(node);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let skip = if position == 0 { 0 } else { prefix };
        let suffix = &entry[skip..];
        page[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    page
}

fn install_indexes(image: &mut [u8], original: Option<&[u8]>, case: &Case, scores: &[i32; 3]) {
    let entries = keys(scores, &case.components);
    let root = case.root as usize;
    let old = |number: usize| original.map(|bytes| &bytes[number * PAGE..(number + 1) * PAGE]);
    if case.node {
        let left = root + 1;
        let right = root + 2;
        let mut bound = if original.is_none() && case.retained_upper {
            key(2500, 2, 1, &case.components)
        } else {
            entries[1].clone()
        };
        bound.extend_from_slice(&(left as u32).to_be_bytes());
        let pages = [
            (
                root,
                index_page(old(root), &[bound], [0, 0, right as u32], true),
            ),
            (
                left,
                index_page(old(left), &entries[..2], [0, right as u32, 0], false),
            ),
            (
                right,
                index_page(old(right), &entries[2..], [left as u32, 0, 0], false),
            ),
        ];
        for (number, page) in pages {
            image[number * PAGE..(number + 1) * PAGE].copy_from_slice(&page);
        }
    } else {
        image[root * PAGE..(root + 1) * PAGE].copy_from_slice(&index_page(
            old(root),
            &entries,
            [0, 0, 0],
            false,
        ));
    }
}

/// Construct only from committed mixed BEFORE images and literal index intent.
pub fn source(header: u8, recipe: Recipe) -> Case {
    let seed = mixed::source(header, recipe.seed());
    assert_eq!(seed.rows.len(), 3);
    for (ordinal, row) in seed.rows.iter().enumerate() {
        assert_eq!(row[0], Value::Int32(ordinal as i32 + 1));
        assert_eq!(row[5], Value::Int32(SCORES[ordinal]));
    }
    let primary = recipe != Recipe::ManualAscending;
    let node = matches!(
        recipe,
        Recipe::CounterTreePrefixLoss | Recipe::CounterTreeRetainedRestore
    );
    let root = if primary { 10 } else { 9 };
    let retained_upper = recipe == Recipe::CounterTreeRetainedRestore;
    let unique = matches!(
        recipe,
        Recipe::CounterUniqueSwap | Recipe::CounterUniqueConflict | Recipe::CounterCompositeTie
    );
    let components = if recipe == Recipe::CounterCompositeTie {
        vec![(5, false), (0, true)]
    } else {
        vec![(5, false)]
    };
    let updates = match recipe {
        Recipe::CounterUniqueSwap => vec![
            Update {
                ordinal: 0,
                expected: 1000,
                replacement: 1999,
            },
            Update {
                ordinal: 2,
                expected: 3000,
                replacement: -2003,
            },
        ],
        Recipe::CounterUniqueConflict | Recipe::CounterCompositeTie => vec![Update {
            ordinal: 0,
            expected: 1000,
            replacement: 999,
        }],
        Recipe::CounterLeafPrefixLoss => vec![Update {
            ordinal: 0,
            expected: 1000,
            replacement: 2_147_482_646,
        }],
        Recipe::CounterTreePrefixLoss => vec![Update {
            ordinal: 2,
            expected: 3000,
            replacement: -3103,
        }],
        _ => vec![Update {
            ordinal: 0,
            expected: 1000,
            replacement: 2599,
        }],
    };
    let final_scores = match recipe {
        Recipe::CounterUniqueSwap => [3000, 2000, 1000],
        Recipe::CounterUniqueConflict | Recipe::CounterCompositeTie => [2000, 2000, 3000],
        Recipe::CounterLeafPrefixLoss => [i32::MAX, 2000, 3000],
        Recipe::CounterTreePrefixLoss => [1000, 2000, -100],
        Recipe::CounterTreeRetainedRestore => SCORES,
        _ => [3600, 2000, 3000],
    };
    let conflict = recipe == Recipe::CounterUniqueConflict;
    let mut case = Case {
        before: seed.before,
        expected: None,
        rows: seed.rows,
        final_rows: None,
        updates,
        changed_rows: if conflict {
            0
        } else {
            SCORES
                .iter()
                .zip(final_scores)
                .filter(|(a, b)| **a != *b)
                .count()
        },
        primary,
        index_name: if components.len() == 2 {
            "ScoreIdIndex"
        } else {
            "ScoreIndex"
        },
        data_number: u32::from(primary),
        root,
        usage_map: (3 << 8) | if primary { 17 } else { 16 },
        owned: if node {
            vec![root, root + 1, root + 2]
        } else {
            vec![root]
        },
        components,
        unique,
        node,
        retained_upper,
        extra: seed.extra,
    };
    let td = definition(&case.before[7 * PAGE..8 * PAGE], &case);
    let maps = append_map(&case.before[3 * PAGE..4 * PAGE], &case.owned);
    case.before[7 * PAGE..8 * PAGE].copy_from_slice(&td);
    case.before[3 * PAGE..4 * PAGE].copy_from_slice(&maps);
    case.before
        .resize((case.owned.last().unwrap() + 1) as usize * PAGE, 0);
    let mut before = case.before.clone();
    install_indexes(&mut before, None, &case, &SCORES);
    case.before = before;
    if !conflict {
        let mut after = case.before.clone();
        let mut final_rows = case.rows.clone();
        for (ordinal, &score) in final_scores.iter().enumerate() {
            final_rows[ordinal][5] = Value::Int32(score);
            let word = 8 * PAGE + 14 + 2 * ordinal;
            let start = 8 * PAGE
                + usize::from(u16::from_le_bytes(
                    case.before[word..word + 2].try_into().unwrap(),
                ));
            assert_eq!(&case.before[start..start + 2], &6_u16.to_le_bytes());
            assert_eq!(
                &case.before[start + 6..start + 10],
                &SCORES[ordinal].to_le_bytes()
            );
            after[start + 6..start + 10].copy_from_slice(&score.to_le_bytes());
        }
        if case.changed_rows != 0 {
            install_indexes(&mut after, Some(&case.before), &case, &final_scores);
        }
        for number in [0, 1, 2, 3, 4, 5, 6, 7] {
            assert_eq!(
                &after[number * PAGE..(number + 1) * PAGE],
                &case.before[number * PAGE..(number + 1) * PAGE]
            );
        }
        if primary {
            assert_eq!(
                &after[9 * PAGE..10 * PAGE],
                &case.before[9 * PAGE..10 * PAGE]
            );
        }
        if retained_upper {
            assert_eq!(after, case.before);
        }
        case.expected = Some(after);
        case.final_rows = Some(final_rows);
    }
    case
}
