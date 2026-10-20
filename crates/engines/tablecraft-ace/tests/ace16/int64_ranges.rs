use super::{budget, int64_index_fixture::database, support};
use proptest::prelude::*;
use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    ops::{
        Bound::{Excluded, Included, Unbounded},
        RangeBounds,
    },
};
use tablecraft_ace::{
    InspectError, Int64IndexSelection as Selection, TableSnapshot, read_table_int64_index_range,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const VALUES: [Option<i64>; 14] = [
    Some(i64::MIN),
    Some(i64::MIN + 1),
    Some(-9_007_199_254_740_993),
    Some(-1),
    Some(0),
    Some(1),
    Some(9_007_199_254_740_992),
    Some(9_007_199_254_740_993),
    Some(9_007_199_254_740_993),
    Some(i64::MAX - 1),
    Some(i64::MAX),
    Some(i64::MAX),
    None,
    None,
];

fn tree(code: u8, descending: bool) -> Vec<u8> {
    let mut bytes = database(code, descending, &VALUES);
    bytes.resize(17 * 4096, 0);
    let original = &bytes[14 * 4096..15 * 4096];
    let mut entries = Vec::new();
    let mut previous = 0;
    for end in 0..4096 - 480 + 1 {
        if original[27 + end / 8] & (1 << (end % 8)) != 0 {
            entries.push(original[480 + previous..480 + end].to_vec());
            previous = end;
        }
    }
    let page = |leaf: bool, entries: &[Vec<u8>], previous: u32, next: u32, tail: u32| {
        let mut raw = vec![0; 4096];
        raw[..2].copy_from_slice(&[if leaf { 4 } else { 3 }, 1]);
        support::put32(&mut raw, 4, 10);
        support::put32(&mut raw, 12, previous);
        support::put32(&mut raw, 16, next);
        support::put32(&mut raw, 20, tail);
        let mut end = 0;
        for entry in entries {
            raw[480 + end..480 + end + entry.len()].copy_from_slice(entry);
            end += entry.len();
            raw[27 + end / 8] |= 1 << (end % 8);
        }
        support::put16(&mut raw, 2, (4096 - 480 - end) as u16);
        raw
    };
    // Split between identical >2^53 keys to exercise duplicate separators.
    let split = if descending { 6 } else { 8 };
    let mut separator = entries[split - 1].clone();
    separator.extend(15_u32.to_be_bytes());
    bytes[14 * 4096..15 * 4096].copy_from_slice(&page(false, &[separator], 0, 0, 16));
    bytes[15 * 4096..16 * 4096].copy_from_slice(&page(true, &entries[..split], 0, 16, 0));
    bytes[16 * 4096..17 * 4096].copy_from_slice(&page(true, &entries[split..], 15, 0, 0));
    bytes[13 * 4096..14 * 4096].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xc0, 1])],
    ));
    bytes
}

fn read(bytes: &[u8], selection: Selection) -> Result<TableSnapshot, InspectError> {
    read_table_int64_index_range(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "BigKey",
        selection,
        &mut budget(),
        &|| false,
    )
}
fn pairs(table: &TableSnapshot) -> Vec<(Option<i64>, i32)> {
    table
        .rows()
        .iter()
        .map(|row| {
            let Value::Int32(id) = row[0] else {
                panic!("ID carrier")
            };
            (
                match row[1] {
                    Value::Int64(key) => Some(key),
                    Value::Null => None,
                    _ => panic!("key carrier"),
                },
                id,
            )
        })
        .collect()
}
fn model(selection: Selection, descending: bool) -> Vec<(Option<i64>, i32)> {
    let mut rows = VALUES
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(ordinal, key)| {
            let matches = match selection {
                Selection::All => true,
                Selection::Null => key.is_none(),
                Selection::Range { lower, upper } => {
                    key.is_some_and(|key| (lower, upper).contains(&key))
                }
            };
            matches.then_some((key, ordinal as i32 + 1))
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        let order = a.0.cmp(&b.0);
        (if descending { order.reverse() } else { order }).then(a.1.cmp(&b.1))
    });
    rows
}

#[test]
fn exact_int64_bounds_keep_nulls_extremes_and_duplicate_separator_rows() {
    for code in [5, 6] {
        for descending in [false, true] {
            let bytes = tree(code, descending);
            for selection in [
                Selection::All,
                Selection::Null,
                Selection::Range {
                    lower: Included(9_007_199_254_740_993),
                    upper: Included(9_007_199_254_740_993),
                },
                Selection::Range {
                    lower: Included(i64::MIN),
                    upper: Excluded(i64::MIN + 1),
                },
                Selection::Range {
                    lower: Excluded(i64::MAX - 1),
                    upper: Included(i64::MAX),
                },
                Selection::Range {
                    lower: Excluded(i64::MAX),
                    upper: Unbounded,
                },
                Selection::Range {
                    lower: Unbounded,
                    upper: Excluded(i64::MIN),
                },
                Selection::Range {
                    lower: Included(1),
                    upper: Excluded(1),
                },
                Selection::Range {
                    lower: Included(2),
                    upper: Included(1),
                },
                Selection::Range {
                    lower: Unbounded,
                    upper: Unbounded,
                },
            ] {
                assert_eq!(
                    pairs(&read(&bytes, selection).unwrap()),
                    model(selection, descending)
                );
            }
        }
    }
    let mut int32 = tree(6, false);
    int32[10 * 4096 + 75 + 25] = 4;
    support::put16(&mut int32, 10 * 4096 + 75 + 25 + 23, 4);
    for selection in [Selection::All, Selection::Null] {
        assert_eq!(
            read(&int32, selection).err(),
            Some(InspectError::Unsupported)
        );
    }
}

struct Touches {
    source: Cursor<Vec<u8>>,
    offsets: Vec<u64>,
}
impl tablecraft_ace::NativeReadSource for Touches {}
impl Read for Touches {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.source.read(buffer)
    }
}
impl Seek for Touches {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        if let SeekFrom::Start(offset) = position {
            self.offsets.push(offset);
        }
        self.source.seek(position)
    }
}

#[test]
fn int64_seek_prunes_unselected_pages_but_fails_visited_corruption_and_limits() {
    let selection = Selection::Range {
        lower: Included(i64::MIN),
        upper: Included(i64::MIN),
    };
    let mut bytes = tree(6, false);
    bytes[16 * 4096] = 0;
    let mut source = Touches {
        source: Cursor::new(bytes.clone()),
        offsets: Vec::new(),
    };
    let selected = read_table_int64_index_range(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "BigKey",
        selection,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(pairs(&selected), model(selection, false));
    assert!(source.offsets.contains(&(15 * 4096)));
    assert!(!source.offsets.contains(&(16 * 4096)));
    assert_eq!(
        read(&bytes, Selection::All).err(),
        Some(InspectError::Malformed)
    );
    bytes[15 * 4096] = 0;
    assert_eq!(read(&bytes, selection).err(), Some(InspectError::Malformed));
    let bytes = tree(6, true);
    for (mut allowance, cancelled) in [(budget(), true), (ResourceBudget::new(66, 1, 100), false)] {
        assert!(
            read_table_int64_index_range(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "BigKey",
                selection,
                &mut allowance,
                &|| cancelled
            )
            .is_err()
        );
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x49363452414e4745),..Default::default()})]
    #[test]
    fn int64_seeks_match_independent_numeric_intervals(lower in any::<i64>(),upper in any::<i64>(),exclude_lower in any::<bool>(),exclude_upper in any::<bool>(),descending in any::<bool>()) {
        let selection=Selection::Range{lower:if exclude_lower{Excluded(lower)}else{Included(lower)},upper:if exclude_upper{Excluded(upper)}else{Included(upper)}};
        prop_assert_eq!(pairs(&read(&tree(6,descending),selection).unwrap()),model(selection,descending));
    }
}
