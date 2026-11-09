use super::*;

fn verify(pair: Pair) {
    assert_eq!(pair.before.len(), pair.after.len());
    assert_eq!(pair.before[20], pair.after[20]);
    let count = 2 * 4096 + 16;
    let slot = 4 * 4096 + 14 + 2 * usize::from(pair.catalog_slot);
    let actual = pair
        .before
        .iter()
        .zip(&pair.after)
        .enumerate()
        .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(actual, [count, slot + 1]);
    assert_eq!(&pair.before[count..count + 4], &[9, 0, 0, 0]);
    assert_eq!(&pair.after[count..count + 4], &[8, 0, 0, 0]);
    assert_eq!(pair.after[slot], pair.before[slot]);
    assert_eq!(pair.after[slot + 1], pair.before[slot + 1] | 0xc0);
    assert_eq!(&pair.before[7 * 4096..], &pair.after[7 * 4096..]);
    assert_eq!(&pair.before[4096..2 * 4096], &pair.after[4096..2 * 4096]);
    assert_eq!(
        &pair.before[3 * 4096..4 * 4096],
        &pair.after[3 * 4096..4 * 4096]
    );
}

#[test]
fn ordinary_table_tombstones_preserve_all_unrelated_and_historical_storage() {
    for header in [5, 6] {
        for name in ["Items", "Inventory", "雪🙂", "\u{feff}Items"] {
            for populated in [false, true] {
                verify(unindexed(header, name, populated));
            }
        }
        for case in [
            Case::CounterPrimary,
            Case::UniqueManual,
            Case::NullableOrdinary,
            Case::AliasHistory,
            Case::RetainedTree,
            Case::InactiveIndexTail,
            Case::UnknownIndexHeader,
        ] {
            verify(indexed(header, case, "Items"));
        }
    }
}

#[test]
fn creator_tombstones_preserve_the_later_database_property_owner() {
    for original in canonical_creator_rename::originals() {
        let pair = creator(&original);
        assert_eq!(pair.catalog_slot, 7);
        verify(pair);
    }
}
