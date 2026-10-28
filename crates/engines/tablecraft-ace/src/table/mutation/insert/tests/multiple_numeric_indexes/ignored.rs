//! IgnoreNULL membership over actual one-, two- and eight-byte DATA carriers.
use super::{Kind, PAGE, Row, model, run};

#[test]
fn allnull_numeric_batches_leave_secondary_tree_maps_and_history_unchanged() {
    for kind in [Kind::Byte, Kind::Int16, Kind::Int64, Kind::Currency] {
        for code in [5, 6] {
            for descending in [false, true] {
                let case = model::ignored(model::source(code, kind, descending, false));
                let requests = [Row { id: 4, group: None }, Row { id: 5, group: None }];
                let after = run(&case, &requests);
                assert_eq!(after.len(), case.before.len());
                assert_eq!(
                    &after[14 * PAGE..16 * PAGE],
                    &case.before[14 * PAGE..16 * PAGE]
                );
                assert_eq!(&after[PAGE..2 * PAGE], &case.before[PAGE..2 * PAGE]);
                assert_eq!(
                    &after[10 * PAGE + 79..10 * PAGE + 83],
                    &29_u32.to_le_bytes()
                );
                assert_eq!(
                    &after[10 * PAGE + 67..10 * PAGE + 71],
                    &19_u32.to_le_bytes()
                );
            }
        }
    }
}

#[test]
fn mixed_numeric_requests_omit_nulls_and_increment_only_new_nonnull_families() {
    for kind in [Kind::Byte, Kind::Int16, Kind::Int64, Kind::Currency] {
        for code in [5, 6] {
            for descending in [false, true] {
                let case = model::ignored(model::source(code, kind, descending, false));
                let minimum = kind.bounds().0;
                let requests = [Some(minimum), None, Some(1), Some(1)]
                    .into_iter()
                    .enumerate()
                    .map(|(position, group)| Row {
                        id: 4 + position as i32,
                        group,
                    })
                    .collect::<Vec<_>>();
                let after = run(&case, &requests);
                assert_eq!(after.len(), case.before.len());
                assert_eq!(
                    &after[10 * PAGE + 79..10 * PAGE + 83],
                    &30_u32.to_le_bytes()
                );
                assert_eq!(
                    &after[10 * PAGE + 67..10 * PAGE + 71],
                    &21_u32.to_le_bytes()
                );
                assert_eq!(
                    &after[15 * PAGE..16 * PAGE],
                    &case.before[15 * PAGE..16 * PAGE]
                );
            }
        }
    }
}
