//! Independent native declarations, complete row histories and protected complements.
use super::{PAGE, alias, creation, duplicate_before, long, source, swapped, word};
use crate::fixtures::access::{
    canonical_retained_add_table::fields, raw_forwarding_chain, secondary_index::normal,
};

fn descriptor(bytes: &[u8], definition: u32) -> usize {
    let base = definition as usize * PAGE;
    let columns = word(bytes, base + 45);
    let mut at = base + 63 + long(bytes, base + 51) as usize * 12 + columns * 25;
    for _ in 0..columns {
        at += 2 + word(bytes, at);
    }
    at
}

fn values(bytes: &[u8], definition: u32) -> Vec<Option<i32>> {
    let data = definition + 1;
    let rows = word(bytes, data as usize * PAGE + 12);
    (0..rows)
        .map(|slot| {
            let row = raw_forwarding_chain::record(bytes, data, slot);
            assert_eq!(row.len(), 7);
            assert_eq!(&row[..2], &[1, 0]);
            if row[6] & 1 != 0 {
                Some(i32::from_le_bytes(row[2..6].try_into().unwrap()))
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn unique_descriptor_is_emitted_directly_and_ordinary_policy_remains_closed() {
    for descending in [false, true] {
        let created = creation(descending, &[Some(1), Some(2), Some(3)]);
        let base = created.target_definition as usize * PAGE;
        assert_eq!(&created.before[base + 47..base + 55], &[0; 8]);
        assert_eq!(
            &created.expected[base + 47..base + 55],
            &[1, 0, 0, 0, 1, 0, 0, 0]
        );
        assert_eq!(
            &created.before[base + 63..base + 88],
            &created.expected[base + 75..base + 100]
        );
        let at = descriptor(&created.expected, created.target_definition);
        assert_eq!(created.expected[at + 46], 0x81);
        assert_eq!(&created.expected[at + 4..at + 6], &[0, 0]);
        assert_eq!(created.expected[at + 6], u8::from(!descending));
        assert_eq!(long(&created.expected, base + 67), 3);
        assert_eq!(
            &created.expected[base + 20..base + 24],
            &created.before[base + 20..base + 24]
        );
        assert_eq!(
            values(&created.expected, created.target_definition),
            [Some(1), Some(2), Some(3)]
        );

        let ordinary = normal::source(normal::Population::NullableDuplicate, descending);
        let at = descriptor(&ordinary.expected, ordinary.target_definition);
        assert_eq!(ordinary.expected[at + 46], 0x80);
    }
}

#[test]
fn each_unique_write_drop_and_post_drop_conflict_has_a_complete_independent_image() {
    for descending in [false, true] {
        let case = source(descending);
        let created = &case.updated.creation;
        let definition = created.target_definition;
        let base = definition as usize * PAGE;
        assert_eq!(
            values(&case.updated.expected, definition),
            [Some(11), Some(12), Some(13)]
        );
        assert_eq!(
            values(&case.inserted, definition),
            [Some(11), Some(12), Some(13), Some(14)]
        );
        assert_eq!(
            values(&case.nulls, definition),
            [Some(11), Some(12), Some(13), Some(14), None, None]
        );
        assert_eq!(
            values(&case.omitted, definition),
            [Some(11), Some(12), Some(13), Some(14), None, None, None]
        );
        assert_eq!(
            values(&case.dropped, definition),
            values(&case.omitted, definition)
        );
        assert_eq!(
            values(&case.duplicate_inserted, definition),
            [
                Some(11),
                Some(12),
                Some(13),
                Some(14),
                None,
                None,
                None,
                Some(12)
            ]
        );
        assert_eq!(
            values(&case.duplicate_updated, definition),
            [
                Some(12),
                Some(12),
                Some(13),
                Some(14),
                None,
                None,
                None,
                Some(12)
            ]
        );
        for (image, count) in [
            (&case.updated.expected, 3),
            (&case.inserted, 4),
            (&case.nulls, 5),
            (&case.omitted, 5),
        ] {
            assert_eq!(long(image, base + 67), count, "historical counter");
            let at = descriptor(image, definition);
            assert_eq!(image[at + 46], 0x81);
        }
        for image in [
            &case.dropped,
            &case.duplicate_inserted,
            &case.duplicate_updated,
        ] {
            assert_eq!(&image[base + 47..base + 55], &[0; 8]);
            assert_eq!(image.len(), case.omitted.len());
            for page in &created.index_pages {
                assert!(
                    image[*page as usize * PAGE..(*page as usize + 1) * PAGE]
                        .iter()
                        .all(|byte| *byte == 0)
                );
            }
        }
        for image in [
            &case.updated.expected,
            &case.inserted,
            &case.nulls,
            &case.omitted,
            &case.dropped,
            &case.duplicate_inserted,
            &case.duplicate_updated,
        ] {
            assert_eq!(
                &image[base + 20..base + 24],
                &created.before[base + 20..base + 24]
            );
            for page in 0..created.before.len() / PAGE {
                if ![1, definition as usize, definition as usize + 1].contains(&page) {
                    assert_eq!(
                        &image[page * PAGE..(page + 1) * PAGE],
                        &created.before[page * PAGE..(page + 1) * PAGE]
                    );
                }
            }
        }
    }
}

#[test]
fn null_ties_and_final_key_swaps_are_distinct_from_conflicting_create_input() {
    for descending in [false, true] {
        let nullable = creation(descending, &[Some(1), None, None]);
        let base = nullable.target_definition as usize * PAGE;
        assert_eq!(long(&nullable.expected, base + 67), 2);
        assert_eq!(
            values(&nullable.expected, nullable.target_definition),
            [Some(1), None, None]
        );
        let swapped = swapped(descending);
        assert_eq!(
            values(&swapped.expected, swapped.creation.target_definition),
            [Some(2), Some(1), Some(3)]
        );
        let base = swapped.creation.target_definition as usize * PAGE;
        assert_eq!(long(&swapped.expected, base + 67), 3);
    }
    let conflicting = duplicate_before();
    let definition = (fields::source(fields::Case::FirstNumber).bytes.len() / PAGE) as u32;
    assert_eq!(
        values(&conflicting, definition),
        [Some(1), Some(1), Some(2)]
    );
    let base = definition as usize * PAGE;
    assert_eq!(&conflicting[base + 47..base + 55], &[0; 8]);
}

#[test]
fn surviving_unique_alias_removes_only_the_selected_logical_declaration() {
    for descending in [false, true] {
        let case = alias(descending);
        let base = case.creation.target_definition as usize * PAGE;
        assert_eq!(
            &case.before[base + 47..base + 55],
            &[2, 0, 0, 0, 1, 0, 0, 0]
        );
        assert_eq!(
            &case.expected[base + 47..base + 55],
            &[1, 0, 0, 0, 1, 0, 0, 0]
        );
        let physical = descriptor(&case.expected, case.creation.target_definition);
        assert_eq!(case.expected[physical + 46], 0x81);
        let logical = physical + 52;
        assert_eq!(long(&case.expected, logical + 4), 1);
        assert_eq!(long(&case.expected, logical + 8), 0);
        for page in 0..case.before.len() / PAGE {
            if page != case.creation.target_definition as usize {
                assert_eq!(
                    &case.before[page * PAGE..(page + 1) * PAGE],
                    &case.expected[page * PAGE..(page + 1) * PAGE]
                );
            }
        }
        assert_eq!(
            values(&case.expected, case.creation.target_definition),
            [Some(1), Some(2), Some(3)]
        );
        assert_eq!(case.before.len(), case.expected.len());
    }
}
