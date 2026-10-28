//! Independent public INSERT images for genuine below-root leaf overflow.
use super::model as base;
use crate::{EditError, InspectError, read_table};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

#[path = "childmodel.rs"]
mod childmodel;
use childmodel::{Case, PAGE, Row, Side};

fn checked(case: &Case, bytes: &[u8], rows: &[Row]) {
    base::checked(bytes, 452 + rows.len(), if case.aliases { 2 } else { 1 });
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut base::budget(),
        &|| false,
    )
    .unwrap();
    for (actual, id) in table.rows().iter().zip(&case.ids) {
        assert_eq!(actual, &[Value::Int32(*id), Value::Null]);
    }
    for (actual, expected) in table.rows()[452..].iter().zip(rows) {
        assert_eq!(actual, &expected.values());
    }
}

fn run(case: &Case, rows: &[Row]) {
    checked(case, &case.before, &[]);
    let (expected, references) = childmodel::expected(case, rows);
    // Decode the complete independent golden before invoking the real service.
    checked(case, &expected, rows);
    assert_eq!(references.len(), rows.len());
    assert_eq!(
        u16::from_le_bytes(expected[13 * PAGE + 2..13 * PAGE + 4].try_into().unwrap()),
        3590
    );
    assert_eq!(expected[13 * PAGE + 26], 1);
    let requests = rows.iter().map(Row::request).collect::<Vec<_>>();
    let candidate = base::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut base::budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.changed_rows(), rows.len());
    assert_eq!(candidate.candidate_bytes(), expected);
    checked(case, candidate.candidate_bytes(), rows);
}

#[test]
fn left_and_right_child_overflow_preserve_full_bounds_rows_and_physical_aliases() {
    for code in [5, 6] {
        for aliases in [false, true] {
            for side in [Side::Left, Side::Right] {
                let case = childmodel::source(code, aliases, side, false);
                run(
                    &case,
                    &[Row {
                        id: childmodel::inserted(side),
                        binary: None,
                    }],
                );
            }
        }
    }
}

#[test]
fn ordinary_eof_precedes_new_left_index_and_aggregates_one_global_byte() {
    for code in [5, 6] {
        for side in [Side::Left, Side::Right] {
            let case = childmodel::source(code, true, side, true);
            let rows = [Row {
                id: childmodel::inserted(side),
                binary: Some(vec![0x80; 255]),
            }];
            let (expected, references) = childmodel::expected(&case, &rows);
            assert_eq!(references, [20 << 8]);
            assert_eq!(expected.len(), 22 * PAGE);
            assert_eq!(&expected[20 * PAGE..20 * PAGE + 2], &[1, 1]);
            assert_eq!(&expected[21 * PAGE..21 * PAGE + 2], &[4, 1]);
            // Global row starts at8123; bitmap byte2 combines DATA20/INDEX21.
            assert_eq!(case.before[8130], 0xf0);
            assert_eq!(expected[8130], 0xc0);
            // Owned DATA row0 and index-owned row each use their own cells.
            assert_eq!(expected[11 * PAGE + 4034] & 0x30, 0x10);
            assert_eq!(expected[14 * PAGE + 4034] & 0x30, 0x20);
            // Candidate lower12 remains, highest15 alone is cleared.
            assert_eq!(expected[11 * PAGE + 3964] & 0x90, 0x10);
            run(&case, &rows);
        }
    }
}

#[test]
fn peer_fit_bulk_binds_request_order_and_repairs_all_three_leaf_links() {
    for code in [5, 6] {
        for side in [Side::Left, Side::Right] {
            for reverse in [false, true] {
                let case = childmodel::source(code, true, side, false);
                let mut rows = vec![
                    Row {
                        id: childmodel::peer(side),
                        binary: Some(Vec::new()),
                    },
                    Row {
                        id: childmodel::inserted(side),
                        binary: None,
                    },
                ];
                if reverse {
                    rows.reverse();
                }
                let (expected, references) = childmodel::expected(&case, &rows);
                assert_eq!(references, [(15 << 8) | 197, (15 << 8) | 198]);
                assert_eq!(expected.len(), 21 * PAGE);
                assert_eq!(&expected[20 * PAGE..20 * PAGE + 2], &[4, 1]);
                run(&case, &rows);
            }
        }
    }
}

#[test]
fn child_split_respects_each_pool_without_publishing_a_candidate() {
    let case = childmodel::source(6, true, Side::Right, true);
    checked(&case, &case.before, &[]);
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut budget = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100_000, 100_000_000),
            Resource::Rows => ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
            Resource::Steps => ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
        };
        let request = Row {
            id: 451,
            binary: None,
        }
        .request();
        let result = base::capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &[request],
            &mut budget,
            &|| false,
        );
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(limit))) if limit.resource == resource)
        );
    }
    run(
        &case,
        &[Row {
            id: 451,
            binary: None,
        }],
    );
}
