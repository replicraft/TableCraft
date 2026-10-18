use super::{budget, copy, put32, two_level_tree};
use proptest::prelude::*;
use std::{
    io::{self, Cursor, Read, Seek, SeekFrom},
    ops::Bound::{Excluded, Included, Unbounded},
};
use tablecraft_ace::{
    InspectError, Int32IndexSelection as Selection, TableSnapshot, read_table_index_range,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn descending_nullable_tree() -> (Vec<u8>, Vec<Vec<Value>>) {
    use std::cmp::Reverse;
    let mut bytes = copy();
    let baseline = read(bytes.clone(), "ORDR_NBR", Selection::All).unwrap();
    let mut entries = Vec::new();
    for (i, original) in baseline.rows().iter().enumerate() {
        let at = 731 * 4096 + 480 + i * 9;
        let reference = u32::from_be_bytes(bytes[at + 5..at + 9].try_into().unwrap());
        assert_eq!(reference >> 8, 728);
        let mut row = original.clone();
        let key = if i % 7 == 0 {
            let slot = (reference & 0xff) as usize;
            let end = if slot == 0 {
                4096
            } else {
                let at = 728 * 4096 + 14 + (slot - 1) * 2;
                (u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) & 0x1fff) as usize
            };
            bytes[728 * 4096 + end - 1] &= !4;
            row[2] = Value::Null;
            None
        } else {
            match row[2] {
                Value::Int32(value) => Some(value),
                _ => panic!("color carrier"),
            }
        };
        let mut encoded = vec![if key.is_none() { 0xff } else { 0x80 }];
        if let Some(key) = key {
            encoded.extend(((key as u32) ^ 0x80000000).to_be_bytes().map(|byte| !byte));
        }
        encoded.extend(reference.to_be_bytes());
        entries.push((Reverse(key), reference, encoded, row));
    }
    entries.sort_by_key(|entry| (entry.0, entry.1));
    let keys: Vec<_> = entries.iter().map(|entry| entry.2.clone()).collect();
    let mut separator = keys[18].clone();
    separator.extend(730_u32.to_be_bytes());
    bytes[729 * 4096..730 * 4096].copy_from_slice(&super::index_page(
        false,
        &[separator],
        0,
        0,
        731,
    ));
    bytes[730 * 4096..731 * 4096].copy_from_slice(&super::index_page(true, &keys[..19], 0, 731, 0));
    bytes[731 * 4096..732 * 4096].copy_from_slice(&super::index_page(true, &keys[19..], 730, 0, 0));
    bytes[726 * 4096 + 236] = 0;
    bytes[727 * 4096 + 3894] = 0x0e;
    (bytes, entries.into_iter().map(|entry| entry.3).collect())
}

fn read(bytes: Vec<u8>, index: &str, selection: Selection) -> Result<TableSnapshot, InspectError> {
    read_table_index_range(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "ROULETTE_TB",
        index,
        selection,
        &mut budget(),
        &|| false,
    )
}

fn ids(result: &TableSnapshot) -> Vec<i32> {
    result
        .rows()
        .iter()
        .map(|row| match row[0] {
            Value::Int32(value) => value,
            _ => panic!("Int32 ID"),
        })
        .collect()
}

struct Touches {
    cursor: Cursor<Vec<u8>>,
    offsets: Vec<u64>,
}
impl Read for Touches {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.cursor.read(bytes)
    }
}
impl Seek for Touches {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if let SeekFrom::Start(offset) = position {
            self.offsets.push(offset);
        }
        self.cursor.seek(position)
    }
}

#[test]
fn equality_ranges_empty_and_unbounded_bounds_return_exact_ids() {
    for (lower, upper, expected) in [
        (Included(4), Included(4), vec![4]),
        (Excluded(4), Included(7), vec![5, 6, 7]),
        (Included(18), Excluded(22), vec![18, 19, 20, 21]),
        (Unbounded, Excluded(3), vec![1, 2]),
        (Included(37), Unbounded, vec![37, 38]),
        (Included(7), Included(4), vec![]),
        (Included(4), Excluded(4), vec![]),
        (Excluded(i32::MAX), Unbounded, vec![]),
        (Unbounded, Excluded(i32::MIN), vec![]),
    ] {
        let result = read(
            two_level_tree(),
            "ORDR_NBR",
            Selection::Range { lower, upper },
        )
        .unwrap();
        assert_eq!(ids(&result), expected);
        assert_eq!(result.schema().declared_rows(), 38);
    }
    assert!(
        read(two_level_tree(), "ORDR_NBR", Selection::Null)
            .unwrap()
            .rows()
            .is_empty()
    );
}

#[test]
fn seek_prunes_disjoint_child_pages_and_fetches_only_selected_rows() {
    let mut source = Touches {
        cursor: Cursor::new(two_level_tree()),
        offsets: Vec::new(),
    };
    let result = read_table_index_range(
        &mut source,
        CatalogScope::new(1),
        "ROULETTE_TB",
        "ORDR_NBR",
        Selection::Range {
            lower: Included(25),
            upper: Included(30),
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(ids(&result), [25, 26, 27, 28, 29, 30]);
    assert!(source.offsets.contains(&(729 * 4096)));
    assert!(!source.offsets.contains(&(730 * 4096)));
    assert!(source.offsets.contains(&(731 * 4096)));
    assert_eq!(
        source
            .offsets
            .iter()
            .filter(|offset| **offset == 728 * 4096)
            .count(),
        6
    );
}

#[test]
fn visited_corruption_and_sibling_boundary_errors_fail_without_full_scan_fallback() {
    let selection = Selection::Range {
        lower: Included(25),
        upper: Included(30),
    };
    let mut bytes = two_level_tree();
    bytes[730 * 4096] = 0;
    assert_eq!(
        ids(&read(bytes.clone(), "ORDR_NBR", selection).unwrap()),
        [25, 26, 27, 28, 29, 30]
    );
    assert_eq!(
        read(bytes, "ORDR_NBR", Selection::All).err(),
        Some(InspectError::Malformed)
    );
    for change in 0..4 {
        let mut bytes = two_level_tree();
        match change {
            0 => bytes[731 * 4096] = 0,
            1 => put32(&mut bytes, 731 * 4096 + 12, 0),
            2 => put32(&mut bytes, 731 * 4096 + 16, 731),
            _ => put32(&mut bytes, 729 * 4096 + 20, 729),
        }
        assert_eq!(
            read(bytes, "ORDR_NBR", selection).err(),
            Some(InspectError::Malformed)
        );
    }
    let mut bytes = two_level_tree();
    put32(&mut bytes, 730 * 4096 + 16, 0);
    assert_eq!(
        read(
            bytes,
            "ORDR_NBR",
            Selection::Range {
                lower: Included(1),
                upper: Included(3)
            }
        )
        .err(),
        Some(InspectError::Malformed)
    );
}

#[test]
fn selection_keeps_missing_index_cancellation_and_row_budget_errors_explicit() {
    let exact = Selection::Range {
        lower: Included(1),
        upper: Included(1),
    };
    assert_eq!(
        read(copy(), "missing", exact).err(),
        Some(InspectError::UnknownIndex)
    );
    let result = read_table_index_range(
        &mut Cursor::new(copy()),
        CatalogScope::new(1),
        "ROULETTE_TB",
        "ORDR_NBR",
        exact,
        &mut ResourceBudget::new(16 * 1024 * 1024, 147, 1_000_000),
        &|| false,
    )
    .unwrap();
    assert_eq!(ids(&result), [1]);
    assert!(
        read_table_index_range(
            &mut Cursor::new(copy()),
            CatalogScope::new(1),
            "ROULETTE_TB",
            "ORDR_NBR",
            Selection::All,
            &mut ResourceBudget::new(16 * 1024 * 1024, 147, 1_000_000),
            &|| false
        )
        .is_err()
    );
    assert_eq!(
        read_table_index_range(
            &mut Cursor::new(copy()),
            CatalogScope::new(1),
            "ROULETTE_TB",
            "ORDR_NBR",
            exact,
            &mut budget(),
            &|| true
        )
        .err(),
        Some(InspectError::Cancelled)
    );
}

#[test]
fn descending_duplicates_across_separators_and_nulls_keep_native_order() {
    use std::ops::RangeBounds;
    let (bytes, model) = descending_nullable_tree();
    for selection in [
        Selection::All,
        Selection::Null,
        Selection::Range {
            lower: Unbounded,
            upper: Unbounded,
        },
        Selection::Range {
            lower: Included(1),
            upper: Included(1),
        },
        Selection::Range {
            lower: Excluded(0),
            upper: Included(2),
        },
        Selection::Range {
            lower: Included(0),
            upper: Excluded(1),
        },
    ] {
        let expected: Vec<_> = model
            .iter()
            .filter(|row| match (selection, &row[2]) {
                (Selection::All, _) | (Selection::Null, Value::Null) => true,
                (Selection::Range { lower, upper }, Value::Int32(value)) => {
                    (lower, upper).contains(value)
                }
                _ => false,
            })
            .cloned()
            .collect();
        let result = read(bytes.clone(), "COLOR_ID", selection).unwrap();
        assert_eq!(result.rows(), expected);
    }
    assert_eq!(model.iter().filter(|row| row[2] == Value::Null).count(), 6);
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5345454b),
        ..Default::default()
    })]
    #[test]
    fn two_level_ranges_match_independent_numeric_filter(a in -5_i32..45, b in -5_i32..45, exclusive_lower in any::<bool>(), exclusive_upper in any::<bool>()) {
        let lower = if exclusive_lower { Excluded(a) } else { Included(a) };
        let upper = if exclusive_upper { Excluded(b) } else { Included(b) };
        let expected: Vec<_> = (1..=38).filter(|value| if exclusive_lower { *value > a } else { *value >= a })
            .filter(|value| if exclusive_upper { *value < b } else { *value <= b }).collect();
        let result = read(two_level_tree(), "ORDR_NBR", Selection::Range { lower, upper }).unwrap();
        prop_assert_eq!(ids(&result), expected);
    }
}
