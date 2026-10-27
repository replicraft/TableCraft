use super::*;
use std::cell::Cell;

#[test]
fn duplicate_and_truncated_known_pairs_fail_while_unknown_carriers_stay_unqualified() {
    let mut duplicate = model::database(6);
    support::put16(&mut duplicate, 10 * 4096 + model::TAIL + 10, 1);
    readable(&duplicate);
    assert_eq!(check(&duplicate).err(), Some(InspectError::Malformed));

    let mut truncated = model::database(6);
    support::put32(&mut truncated, 10 * 4096 + 8, (model::TAIL + 9) as u32);
    readable(&truncated);
    assert_eq!(check(&truncated).err(), Some(InspectError::Truncated));

    let mut flags = model::database(6);
    flags[10 * 4096 + 103] = 0;
    readable(&flags);
    assert_eq!(
        check(&flags).unwrap().columns()[0].status(),
        LongValueCheckStatus::UnsupportedColumn
    );

    let mut unknown_map = model::database(6);
    let mut map = model::map(&[14, 15, 16, 20]);
    map[0] = 7;
    model::maps(&mut unknown_map, map);
    readable(&unknown_map);
    assert_eq!(
        check(&unknown_map).unwrap().columns()[0].status(),
        LongValueCheckStatus::UnknownMap
    );
}

#[test]
fn budgets_and_late_cancellation_return_no_prefix_and_allow_deterministic_retry() {
    let bytes = model::database(6);
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 10_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 10_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            check_table_long_values(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut allowance,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    let calls = Cell::new(0);
    check_table_long_values(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let total = calls.get();
    assert!(total > 20);
    calls.set(0);
    assert!(matches!(
        check_table_long_values(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == total - 1
            }
        ),
        Err(InspectError::Cancelled)
    ));
    assert_eq!(check(&bytes).unwrap().columns()[0].distinct_chunks(), 3);
}
