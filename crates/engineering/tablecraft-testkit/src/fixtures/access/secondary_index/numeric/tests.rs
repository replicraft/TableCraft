//! Literal category goldens and raw ROW/fence checks, without native readers.
use super::*;
use std::collections::BTreeMap;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn record(bytes: &[u8], reference: u32) -> &[u8] {
    let page = (reference >> 8) as usize * 4096;
    let slot = (reference & 255) as usize;
    assert!(slot < word(bytes, page + 12));
    let start = word(bytes, page + 14 + 2 * slot) & 8191;
    let end = if slot == 0 {
        4096
    } else {
        word(bytes, page + 12 + 2 * slot) & 8191
    };
    &bytes[page + start..page + end]
}

#[test]
fn literal_key_bytes_preserve_boolean_unsigned_scaled_and_signed_zero_categories() {
    for (kind, value, expected) in [
        (Kind::Boolean, -1, vec![127, 0]),
        (Kind::Boolean, 0, vec![127, 255]),
        (Kind::Byte, 255, vec![127, 255]),
        (Kind::Int16, -32_768, vec![127, 0, 0]),
        (Kind::Int32, -2_147_483_648, vec![127, 0, 0, 0, 0]),
        (Kind::Int64, i64::MIN, vec![127, 0, 0, 0, 0, 0, 0, 0, 0]),
        (
            Kind::Currency,
            12_345,
            vec![127, 128, 0, 0, 0, 0, 0, 48, 57],
        ),
        (Kind::Single, 0x8000_0000, vec![127, 127, 255, 255, 255]),
        (Kind::Single, 0, vec![127, 128, 0, 0, 0]),
        (
            Kind::Double,
            i64::MIN,
            vec![127, 127, 255, 255, 255, 255, 255, 255, 255],
        ),
        (Kind::Double, 0, vec![127, 128, 0, 0, 0, 0, 0, 0, 0]),
        (
            Kind::DateTime,
            0x4004_0000_0000_0000,
            vec![127, 192, 4, 0, 0, 0, 0, 0, 0],
        ),
    ] {
        assert_eq!(key(kind, Some(value), false), expected);
        assert_eq!(
            key(kind, Some(value), true),
            expected.iter().map(|byte| !byte).collect::<Vec<_>>()
        );
    }
    for kind in KINDS.into_iter().filter(|kind| *kind != Kind::Boolean) {
        assert_eq!(key(kind, None, false), [0]);
        assert_eq!(key(kind, None, true), [255]);
    }
    assert!(std::panic::catch_unwind(|| key(Kind::Boolean, None, false)).is_err());
}

#[test]
fn every_scalar_direction_preserves_original_records_companions_and_old_index_pages() {
    for kind in KINDS {
        for descending in [false, true] {
            let source = source(kind, Population::NullableDuplicate, descending);
            let definition = source.target_definition as usize * PAGE;
            assert_eq!(word(&source.before, definition + 45), 4);
            assert_eq!(source.before[definition + 75], kind.native());
            assert_eq!(long(&source.before, definition + 47), 1);
            assert_eq!(long(&source.expected, definition + 47), 2);
            assert_eq!(
                source.rows.len(),
                values(kind, Population::NullableDuplicate).len()
            );
            for row in &source.rows {
                assert_eq!(
                    record(&source.before, row.reference),
                    record(&source.expected, row.reference)
                );
                if kind == Kind::Boolean {
                    assert_eq!(&record(&source.before, row.reference)[..2], &[4, 0]);
                    assert_eq!(
                        record(&source.before, row.reference).last().unwrap() & 1 != 0,
                        row.value == Some(-1)
                    );
                }
            }
            // Every old non-definition/global-map page, including the existing
            // Guard tree/map and Unicode/Currency DATA, is protected wholesale.
            for page in 0..source.before.len() / PAGE {
                if page != 1 && page != source.target_definition as usize {
                    assert_eq!(
                        &source.before[page * PAGE..(page + 1) * PAGE],
                        &source.expected[page * PAGE..(page + 1) * PAGE]
                    );
                }
            }
            audit_tree(&source);
            let empty = super::source(kind, Population::Empty, descending);
            assert!(empty.rows.is_empty());
            assert_eq!(empty.leaf_pages, 1);
            audit_tree(&empty);
        }
    }
}

fn wires(page: &[u8]) -> Vec<Vec<u8>> {
    let used = 3616 - word(page, 2);
    let prefix = word(page, 24);
    let mut entries = Vec::new();
    let mut start = 0;
    for end in 1..=used {
        if page[27 + end / 8] & (1 << (end % 8)) == 0 {
            continue;
        }
        let mut wire = if entries.is_empty() {
            Vec::new()
        } else {
            page[480..480 + prefix].to_vec()
        };
        wire.extend(&page[480 + start..480 + end]);
        entries.push(wire);
        start = end;
    }
    assert_eq!(start, used);
    entries
}

fn audit_tree(source: &Source) {
    let mut expected = source
        .rows
        .iter()
        .map(|row| {
            let mut value = key(source.kind, row.value, source.descending);
            value.extend(row.reference.to_be_bytes());
            value
        })
        .collect::<Vec<_>>();
    expected.sort();
    let mut actual = Vec::new();
    let mut maxima = BTreeMap::new();
    for number in &source.index_pages {
        let page = &source.expected[*number as usize * PAGE..(*number as usize + 1) * PAGE];
        assert_eq!(long(page, 4), source.target_definition);
        for (at, reverse) in [(12, 16), (16, 12)] {
            let sibling = long(page, at);
            if sibling != 0 {
                let sibling_page =
                    &source.expected[sibling as usize * PAGE..(sibling as usize + 1) * PAGE];
                assert_eq!(long(sibling_page, reverse), *number);
                assert_eq!(sibling_page[26], page[26]);
            }
        }
        let rows = wires(page);
        if page[0] == 4 {
            assert_eq!(page[26], 0);
            if let Some(last) = rows.last() {
                maxima.insert(*number, last.to_vec());
            }
            actual.extend(rows);
        } else {
            assert_eq!(page[0], 3);
            assert!(page[26] > 0);
            for row in rows {
                let length = row.len();
                let child = u32::from_be_bytes(row[length - 4..].try_into().unwrap());
                assert_eq!(&row[..length - 4], maxima.get(&child).unwrap().as_slice());
            }
            let tail = long(page, 20);
            maxima.insert(*number, maxima.get(&tail).unwrap().to_vec());
        }
    }
    assert_eq!(actual, expected);
    assert_eq!(source.root, *source.index_pages.last().unwrap());
    assert_eq!(
        source.expected[source.root as usize * PAGE + 26] as usize + 1,
        source.levels
    );
}

#[test]
fn eight_byte_populations_exercise_more_than_two_leaves_and_real_parent_levels() {
    for (population, minimum_levels) in [(Population::ThreeLeaves, 2), (Population::MultiLevel, 3)]
    {
        for descending in [false, true] {
            let source = source(Kind::Int64, population, descending);
            assert!(source.leaf_pages > 2);
            assert!(source.levels >= minimum_levels);
            audit_tree(&source);
        }
    }
}
