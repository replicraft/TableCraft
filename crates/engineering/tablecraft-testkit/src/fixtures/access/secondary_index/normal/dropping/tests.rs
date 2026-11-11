//! Independent retirement scope is larger than the old pre-CREATE file.
use super::*;
use crate::fixtures::access::secondary_index::normal::{self, Population};

fn check(source: &Source) {
    let result = after(source);
    let before = &source.expected;
    assert_eq!(result.len(), before.len());
    assert!(result.len() > source.before.len());
    let mut retired = source.index_pages.to_vec();
    retired.push(source.map_host);
    retired.sort_unstable();
    assert!(retired.contains(&source.root));
    for (page, (old, new)) in before
        .as_chunks::<PAGE>()
        .0
        .iter()
        .zip(result.as_chunks::<PAGE>().0)
        .enumerate()
    {
        if retired.binary_search(&(page as u32)).is_ok() {
            assert!(new.iter().all(|byte| *byte == 0));
        } else if page != 1 && page != source.target_definition as usize {
            assert!(new == old, "protected page {page}");
        }
    }
    let root = source.target_definition as usize * PAGE;
    let old = &before[root..root + PAGE];
    let new = &result[root..root + PAGE];
    assert_eq!(long(new, 47) + 1, long(old, 47));
    assert_eq!(long(new, 51) + 1, long(old, 51));
    assert_eq!(
        &new[16..47],
        &old[16..47],
        "rows/LAST/native field highwaters unchanged"
    );
    let end = long(new, 8);
    let original_end = long(old, 8);
    assert!(new[end..original_end].iter().all(|byte| *byte == 0));
    assert_eq!(
        &new[original_end..],
        &old[original_end..],
        "preexisting tail protected"
    );
    assert_eq!(
        &new[..end],
        &source.before[root..root + end],
        "active declaration inverse only, never wholeimage inverse"
    );
    let start = PAGE + word(before, PAGE + 14);
    for page in 0..before.len() / PAGE {
        let cell = start + 5 + page / 8;
        let bit = 1 << (page % 8);
        if cell >= 2 * PAGE {
            break;
        }
        let expected = before[cell] & bit != 0 || retired.binary_search(&(page as u32)).is_ok();
        assert_eq!(result[cell] & bit != 0, expected, "globalfree page {page}");
    }
}

#[test]
fn all_tree_levels_and_directions_retire_exact_exclusive_footprint() {
    for descending in [false, true] {
        for population in [
            Population::Empty,
            Population::NullableDuplicate,
            Population::ThreeLeaves,
            Population::MultiLevel,
        ] {
            check(&normal::source(population, descending));
        }
        check(&normal::protected_companion(descending));
    }
}

#[test]
fn surviving_alias_removes_only_logical_name_and_keeps_physical_pages_and_identity() {
    for source in [
        normal::source(Population::NullableDuplicate, false),
        normal::protected_companion(true),
    ] {
        let alias = surviving_alias(&source, "ScoreAlias");
        let base = source.target_definition as usize * PAGE;
        for page in 0..alias.before.len() / PAGE {
            if page != source.target_definition as usize {
                assert_eq!(
                    &alias.before[page * PAGE..(page + 1) * PAGE],
                    &alias.expected[page * PAGE..(page + 1) * PAGE]
                );
            }
        }
        assert_eq!(
            long(&alias.before, base + 51),
            long(&alias.expected, base + 51)
        );
        assert_eq!(
            long(&alias.before, base + 47) - 1,
            long(&alias.expected, base + 47)
        );
        assert_eq!(
            &alias.before[base + 16..base + 47],
            &alias.expected[base + 16..base + 47]
        );
        for page in source.index_pages.iter().copied().chain([source.map_host]) {
            assert!(
                alias.expected[page as usize * PAGE..(page as usize + 1) * PAGE]
                    .iter()
                    .any(|byte| *byte != 0)
            );
        }
        let page = &alias.expected[base..base + PAGE];
        let physical = long(page, 51);
        let logical = long(page, 47);
        let mut start = 63 + physical * 12 + word(page, 45) * 25;
        for _ in 0..word(page, 45) {
            start += 2 + word(page, start);
        }
        start += physical * 52;
        let survivors = (0..logical)
            .map(|position| long(page, start + position * 28 + 4))
            .collect::<Vec<_>>();
        assert_eq!(
            survivors.last().copied(),
            Some(logical),
            "new alias retains a nondense logical identity"
        );
    }
}

#[test]
fn dropping_first_physical_compacts_surviving_primary_reference_without_reidentifying_it() {
    let original = normal::protected_companion(false);
    let reordered = reordered_physical(&original);
    let base = original.target_definition as usize * PAGE;
    for page in 0..original.expected.len() / PAGE {
        if page != original.target_definition as usize {
            assert!(
                original.expected[page * PAGE..(page + 1) * PAGE]
                    == reordered.expected[page * PAGE..(page + 1) * PAGE]
            );
        }
    }
    check(&reordered);
    let dropped = after(&reordered);
    // The original primary declaration must now occupy physical0 again while
    // keeping its original logical0 identity and exact root/map/key metadata.
    let length = long(&original.before, base + 8);
    assert!(dropped[base..base + length] == original.before[base..base + length]);
}
