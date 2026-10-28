//! Literal complete-image controls for general height-one INSERT routing.
use super::model as base;
use crate::{EditError, IndexConstraintViolation, InspectError, read_table};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

mod model;
use model::{Case, Expected, PAGE, Row};

fn checked(case: &Case, bytes: &[u8], rows: &[Row]) {
    base::checked(
        bytes,
        case.rows.len() + rows.len(),
        if case.aliases { 2 } else { 1 },
    );
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut base::budget(),
        &|| false,
    )
    .unwrap();
    for (actual, (id, binary)) in table.rows().iter().zip(&case.rows) {
        assert_eq!(
            actual,
            &[
                Value::Int32(*id),
                binary
                    .as_ref()
                    .map_or(Value::Null, |bytes| Value::Binary(bytes.clone()))
            ]
        );
    }
    for (actual, row) in table.rows()[case.rows.len()..].iter().zip(rows) {
        assert_eq!(actual, &row.values());
    }
}

fn run(case: &Case, rows: &[Row]) -> Expected {
    checked(case, &case.before, &[]);
    let expected = model::expected(case, rows).expect("independently fitting parent");
    checked(case, &expected.bytes, rows);
    let requests = rows.iter().map(Row::request).collect::<Vec<_>>();
    let candidate = base::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut base::budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.changed_rows(), rows.len());
    assert_eq!(candidate.candidate_bytes(), expected.bytes);
    checked(case, candidate.candidate_bytes(), rows);
    expected
}

fn null(id: i32) -> Row {
    Row { id, binary: None }
}

fn centered(center: i32) -> Vec<i32> {
    (center - 226..center)
        .chain(center + 1..=center + 225)
        .collect()
}

#[test]
fn one_two_three_and_five_children_preserve_tail_and_bounded_forms() {
    for code in [5, 6] {
        for bounded in [false, true] {
            for count in [1, 2, 3, 5] {
                let groups = (0..count)
                    .map(|index| vec![index * 100, index * 100 + 2])
                    .collect::<Vec<_>>();
                let case = model::source(code, true, &groups, bounded, 0, false);
                let rows = [
                    null(1),
                    Row {
                        id: i32::MAX,
                        binary: Some(Vec::new()),
                    },
                ];
                let expected = run(&case, &rows);
                assert_eq!(expected.branches.len(), count as usize);
                assert_eq!(expected.bytes.len(), case.before.len());
                let tail = u32::from_le_bytes(
                    expected.bytes[13 * PAGE + 20..13 * PAGE + 24]
                        .try_into()
                        .unwrap(),
                );
                assert_eq!(tail == 0, bounded);
            }
        }
    }
}

#[test]
fn independently_derived_three_leaf_result_is_a_real_next_insert_source() {
    for code in [5, 6] {
        for bounded in [false, true] {
            let case = model::source(
                code,
                true,
                &[centered(0), vec![1_000_000_000]],
                bounded,
                0,
                false,
            );
            let rows = [null(0)];
            let first = run(&case, &rows);
            assert_eq!(first.branches.len(), 3);
            let next = model::next(&case, &rows, first);
            run(
                &next,
                &[Row {
                    id: i32::MAX,
                    binary: Some(vec![0x81; 255]),
                }],
            );
        }
    }
}

#[test]
fn three_original_offenders_complete_each_local_queue_in_logical_order() {
    for code in [5, 6] {
        let case = model::source(
            code,
            true,
            &[centered(0), centered(1 << 24), centered(2 << 24)],
            false,
            0,
            false,
        );
        let rows = [null(0), null(1 << 24), null(2 << 24)];
        let expected = model::expected(&case, &rows).unwrap();
        let eof = (case.before.len() / PAGE) as u32;
        assert_eq!(
            expected
                .branches
                .iter()
                .map(|branch| branch.number)
                .collect::<Vec<_>>(),
            [eof, 32, eof + 1, 33, eof + 2, 34]
        );
        assert!(
            expected
                .branches
                .iter()
                .all(|branch| branch.entries.len() == 226)
        );
        run(&case, &rows);
    }
}

#[test]
fn repeated_odd_split_preserves_origin_seeds_and_data_first_creation_order() {
    for code in [5, 6] {
        let case = model::source(code, true, &[(0..600).collect()], false, 0, false);
        let rows = (0..152)
            .map(|index| null(-1_000_000_000 + index * 1_000_000))
            .chain((0..153).map(|index| null(1_000_000_000 + index * 1_000_000)))
            .collect::<Vec<_>>();
        let expected = model::expected(&case, &rows).unwrap();
        let eof = (case.before.len() / PAGE) as u32;
        assert_eq!(expected.references[165], eof << 8);
        assert_eq!(
            expected
                .branches
                .iter()
                .map(|branch| branch.number)
                .collect::<Vec<_>>(),
            [eof + 3, eof + 1, eof + 2, 32]
        );
        assert_eq!(
            expected
                .branches
                .iter()
                .map(|branch| branch.entries.len())
                .collect::<Vec<_>>(),
            [226, 226, 226, 227]
        );
        for branch in &expected.branches[..3] {
            assert_eq!(
                &expected.bytes
                    [branch.number as usize * PAGE + 24..branch.number as usize * PAGE + 26],
                &[0, 0]
            );
        }
        assert_eq!(&expected.bytes[32 * PAGE + 24..32 * PAGE + 26], &[1, 0]);
        run(&case, &rows);
    }
}

#[test]
fn index_bitmap_is_separate_from_members_and_aggregates_the_global_byte() {
    for code in [5, 6] {
        let case = model::source(
            code,
            true,
            &[centered(0), vec![1_000_000_000]],
            false,
            0,
            true,
        );
        let expected = model::expected(&case, &[null(0)]).unwrap();
        assert_eq!(expected.bitmap, Some(601));
        assert_eq!(expected.bytes.len(), 602 * PAGE);
        assert_eq!(expected.branches[0].number, 600);
        assert_eq!(expected.bytes[599 * PAGE + 4 + 75], 0xfc);
        assert_eq!(expected.bytes[601 * PAGE + 4 + 75], 1);
        assert_eq!(
            &expected.bytes[598 * PAGE..599 * PAGE],
            &case.before[598 * PAGE..599 * PAGE]
        );
        assert_eq!(expected.bytes[597 * PAGE + 4 + 75] & 3, 0);
        run(&case, &[null(0)]);
    }
}

#[test]
fn parent_capacity_and_prefix_loss_grow_root_with_exact_preserved_sources() {
    for code in [5, 6] {
        let mut groups = (0..301)
            .map(|index| {
                let value = -1_500_000_000_i64 + i64::from(index) * 10_000_000;
                vec![i32::try_from(value).unwrap()]
            })
            .collect::<Vec<_>>();
        groups[150] = centered(0);
        groups.push(vec![1_600_000_000]);
        let cases = [
            (model::source(code, true, &groups, false, 1, false), null(0)),
            (
                model::source(
                    code,
                    true,
                    &(0..361).map(|id| vec![id]).collect::<Vec<_>>(),
                    true,
                    3,
                    false,
                ),
                null(i32::MAX),
            ),
        ];
        for (case, row) in cases {
            checked(&case, &case.before, &[]);
            assert_eq!(&case.before[13 * PAGE + 2..13 * PAGE + 4], &[3, 0]);
            // Prove the same large source is admitted for a fitting change
            // before attributing the failing request to parent capacity.
            let fitting = if row.id == i32::MAX {
                null(361)
            } else {
                null(-1_499_999_999)
            };
            run(&case, &[fitting]);
            assert!(model::expected(&case, std::slice::from_ref(&row)).is_none());
            let expected = super::recursive_tree::expected_height_one(
                &case.before,
                &[(row.id, row.binary.clone())],
                case.aliases,
            );
            checked(&case, &expected, std::slice::from_ref(&row));
            assert_eq!(expected[13 * PAGE + 26], 2);
            let result = base::capture(&case.before)
                .preview_ace16_insert_rows("Items", &[row.request()], &mut base::budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(result.changed_rows(), 1);
            assert_eq!(result.candidate_bytes(), expected);
        }
    }
}

#[test]
fn pools_late_cancellation_duplicates_and_empty_requests_keep_original_authority() {
    let case = model::source(
        6,
        true,
        &[centered(0), vec![1_000_000_000]],
        false,
        0,
        false,
    );
    checked(&case, &case.before, &[]);
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut budget = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100_000, 100_000_000),
            Resource::Rows => ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
            Resource::Steps => ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
        };
        let result = base::capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &[null(0).request()],
            &mut budget,
            &|| false,
        );
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(limit))) if limit.resource == resource)
        );
    }
    let calls = Cell::new(0_usize);
    let expected = model::expected(&case, &[null(0)]).unwrap();
    let candidate = base::capture(&case.before)
        .preview_ace16_insert_rows("Items", &[null(0).request()], &mut base::budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), expected.bytes);
    let threshold = calls.get().saturating_sub(3);
    assert!(threshold > 0);
    let current = Cell::new(0);
    let cancelled = base::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &[null(0).request()],
        &mut base::budget(),
        &|| {
            current.set(current.get() + 1);
            current.get() >= threshold
        },
    );
    assert_eq!(
        cancelled.err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    let duplicate = base::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &[null(-1).request()],
        &mut base::budget(),
        &|| false,
    );
    assert_eq!(
        duplicate.err(),
        Some(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        )))
    );
    let empty = base::capture(&case.before)
        .preview_ace16_insert_rows("Items", &[], &mut base::budget(), &|| false)
        .unwrap();
    assert_eq!(empty.candidate_bytes(), case.before);
    assert_eq!(empty.changed_rows(), 0);
}
