//! Complete independent images for recursive INSERT allocation and framing.
use super::model as base;
use crate::{EditError, InspectError, WriteBlocker, read_table};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

mod model;
use model::{Case, Expected, Maps, PAGE, Row, Shape};

fn null(id: i32) -> Row {
    Row { id, binary: None }
}

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
    let expected = model::expected(case, rows);
    // Decode every record, key, ROW, membership and bound before the real API.
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

/// Direct shared-source boundary for the two preserved, old H1-only controls.
pub(super) fn expected_height_one(
    before: &[u8],
    rows: &[(i32, Option<Vec<u8>>)],
    aliases: bool,
) -> Vec<u8> {
    let case = model::height_one_source(before, aliases);
    let requests = rows
        .iter()
        .map(|(id, binary)| Row {
            id: *id,
            binary: binary.clone(),
        })
        .collect::<Vec<_>>();
    let expected = model::expected(&case, &requests);
    checked(&case, &expected.bytes, &requests);
    expected.bytes
}

#[test]
fn prefix_loss_grows_retained_root_with_right_then_left_and_real_next_source() {
    for code in [5, 6] {
        for aliases in [false, true] {
            for value in [i32::MAX - 1, i32::MAX] {
                let case = model::source(code, Shape::PrefixLoss, Maps::Inline, aliases);
                assert_eq!(&case.before[13 * PAGE + 2..13 * PAGE + 4], &[3, 0]);
                let expected = run(&case, &[null(value)]);
                assert_eq!(expected.bytes[13 * PAGE + 26], 2);
                assert_eq!(expected.levels[1], [412, 411]);
                assert_eq!(expected.fresh, [411, 412]);
                assert_eq!(
                    expected
                        .nodes
                        .iter()
                        .find(|node| node.number == 412)
                        .unwrap()
                        .children
                        .len(),
                    180
                );
                assert_eq!(
                    expected
                        .nodes
                        .iter()
                        .find(|node| node.number == 411)
                        .unwrap()
                        .children
                        .len(),
                    181
                );
                assert_eq!(&expected.bytes[13 * PAGE + 20..13 * PAGE + 26], &[0; 6]);
                if value != i32::MAX {
                    let next = model::next(&case, &[null(value)], expected);
                    let fitted = run(&next, &[null(-1), null(i32::MAX)]);
                    assert!(fitted.fresh.is_empty());
                    assert_eq!(fitted.bytes.len(), next.before.len());
                    let noop = base::capture(&next.before)
                        .preview_ace16_insert_rows("Items", &[], &mut base::budget(), &|| false)
                        .unwrap();
                    assert_eq!(noop.candidate_bytes(), next.before);
                }
            }
        }
    }
}

#[test]
fn nonroot_internal_split_and_all_height_links_preserve_original_right() {
    for code in [5, 6] {
        let case = model::source(code, Shape::TwoParents, Maps::Inline, true);
        let expected = run(&case, &[null(1_000_450)]);
        assert_eq!(expected.fresh, [414, 415]);
        assert_eq!(expected.levels[1], [415, 20, 21]);
        assert_eq!(expected.bytes[13 * PAGE + 26], 2);
        assert_eq!(
            expected
                .nodes
                .iter()
                .find(|node| node.number == 415)
                .unwrap()
                .children
                .len(),
            181
        );
        assert_eq!(
            expected
                .nodes
                .iter()
                .find(|node| node.number == 20)
                .unwrap()
                .children
                .len(),
            181
        );
        assert_eq!(&expected.bytes[20 * PAGE + 24..20 * PAGE + 26], &[2, 0]);
        assert_eq!(&expected.bytes[415 * PAGE + 24..415 * PAGE + 26], &[0, 0]);
        assert_eq!(
            &expected.bytes[20 * PAGE + 20..20 * PAGE + 24],
            &case.before[20 * PAGE + 20..20 * PAGE + 24]
        );
    }
}

#[test]
fn three_offenders_under_distinct_parents_and_height_three_clamp_fit() {
    for code in [5, 6] {
        let case = model::source(code, Shape::ThreeOffenders, Maps::Inline, true);
        let expected = run(&case, &[null(0), null(1 << 24), null(2 << 24)]);
        assert_eq!(expected.fresh, [128, 129, 130]);
        assert_eq!(expected.levels[0], [128, 32, 129, 33, 130, 34]);
        assert_eq!(expected.levels[1], [20, 21]);
        let tall = model::source(code, Shape::HeightThree, Maps::Inline, true);
        let fitted = run(
            &tall,
            &[
                Row {
                    id: -1,
                    binary: Some(Vec::new()),
                },
                null(i32::MAX),
            ],
        );
        assert_eq!(fitted.bytes[13 * PAGE + 26], 3);
        assert!(fitted.fresh.is_empty());
    }
}

#[test]
fn data_first_root_growth_and_rebase_then_bitmap_promotion_keep_roles_disjoint() {
    for code in [5, 6] {
        let case = model::source(code, Shape::PrefixLoss, Maps::Inline, true);
        let rows = (361..700)
            .map(null)
            .chain([null(i32::MAX)])
            .collect::<Vec<_>>();
        let expected = run(&case, &rows);
        assert_eq!(expected.references[149], 411 << 8);
        assert_eq!(expected.fresh, [412, 413]);
        assert_eq!(&expected.bytes[411 * PAGE..411 * PAGE + 2], &[1, 1]);
        assert_eq!(expected.bytes[PAGE + 4027 + 5 + 51], 0xc0);
        let case = model::source(code, Shape::PrefixLoss, Maps::At600, true);
        let promoted = run(&case, &[null(i32::MAX)]);
        assert_eq!(promoted.fresh, [600, 602]);
        assert_eq!(promoted.bitmaps, [601]);
        assert_eq!(promoted.bytes[601 * PAGE + 4 + 75] & 7, 5);
        assert_eq!(promoted.bytes[597 * PAGE + 4 + 75] & 7, 0);
        let case = model::source(code, Shape::TwoParents, Maps::RebaseThenPromote, true);
        let late = run(&case, &[null(1_000_450)]);
        assert_eq!(late.fresh, [518, 519]);
        assert!(late.bitmaps.is_empty());
        let map = 14 * PAGE + 4027;
        assert_eq!(late.bytes[map], 0);
        assert_eq!(&late.bytes[map + 1..map + 5], &8_u32.to_le_bytes());
        assert_eq!(late.bytes[map + 68] & 0xc0, 0xc0);
        assert_eq!(
            &late.bytes[516 * PAGE..517 * PAGE],
            &case.before[516 * PAGE..517 * PAGE]
        );
        // 519 is still the last represented cell of the rebased window.
        // A third actual allocation at520, not519, forces a bitmap at521.
        let case = model::source(code, Shape::ThreeOffenders, Maps::RebaseThenPromote, true);
        let promoted = run(&case, &[null(0), null(1 << 24), null(2 << 24)]);
        assert_eq!(promoted.fresh, [518, 519, 520]);
        assert_eq!(promoted.bitmaps, [521]);
        assert_eq!(promoted.bytes[map], 1);
        assert_eq!(&promoted.bytes[map + 1..map + 5], &521_u32.to_le_bytes());
        assert_eq!(promoted.bytes[521 * PAGE + 4 + 64] & 0xc0, 0xc0);
        assert_eq!(promoted.bytes[521 * PAGE + 4 + 65] & 3, 1);
        assert_eq!(promoted.bytes[515 * PAGE + 4 + 64] & 0xc0, 0);
        assert_eq!(promoted.bytes[515 * PAGE + 4 + 65] & 3, 0);
        assert_eq!(promoted.bytes[517 * PAGE + 4 + 64], 0);
        assert_eq!(promoted.bytes[517 * PAGE + 4 + 65], 0xfc);
        assert_eq!(
            &promoted.bytes[516 * PAGE..517 * PAGE],
            &case.before[516 * PAGE..517 * PAGE]
        );
    }
}

#[test]
fn root_leaf_bulk_uses_fresh_right_first_and_more_than_two_fitting_halves() {
    for code in [5, 6] {
        let case = model::source(code, Shape::RootLeaf, Maps::Inline, true);
        let rows = (0..1500)
            .map(|index| {
                null(i32::try_from(-1_500_000_000_i64 + i64::from(index) * 2_000_000).unwrap())
            })
            .collect::<Vec<_>>();
        let expected = run(&case, &rows);
        assert_eq!(expected.bytes[13 * PAGE + 26], 1);
        assert_eq!(expected.levels[0], [136, 134, 135, 133]);
        assert_eq!(expected.fresh, [133, 134, 135, 136]);
        assert_eq!(
            expected
                .nodes
                .iter()
                .filter(|node| node.height == 0)
                .map(|node| node.entries.len())
                .sum::<usize>(),
            1503
        );
        assert_ne!(&expected.bytes[13 * PAGE + 20..13 * PAGE + 24], &[0; 4]);
        for number in &expected.fresh {
            assert_eq!(
                &expected.bytes[*number as usize * PAGE + 24..*number as usize * PAGE + 26],
                &[0, 0]
            );
        }
    }
}

#[test]
fn recursive_candidates_respect_all_pools_late_cancel_and_default_gate() {
    let case = model::source(6, Shape::PrefixLoss, Maps::Inline, true);
    checked(&case, &case.before, &[]);
    let expected = model::expected(&case, &[null(i32::MAX)]);
    checked(&case, &expected.bytes, &[null(i32::MAX)]);
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut budget = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100_000, 100_000_000),
            Resource::Rows => ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
            Resource::Steps => ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
        };
        let result = base::capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &[null(i32::MAX).request()],
            &mut budget,
            &|| false,
        );
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(limit))) if limit.resource == resource)
        );
    }
    let calls = Cell::new(0_usize);
    let candidate = base::capture(&case.before)
        .preview_ace16_insert_rows(
            "Items",
            &[null(i32::MAX).request()],
            &mut base::budget(),
            &|| {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), expected.bytes);
    let threshold = calls.get() - 3;
    let current = Cell::new(0_usize);
    let failed = base::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &[null(i32::MAX).request()],
        &mut base::budget(),
        &|| {
            current.set(current.get() + 1);
            current.get() >= threshold
        },
    );
    assert_eq!(
        failed.err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    let default = base::source(6, true, false, Some(("DefaultValue", "0")));
    base::checked(&default, 3, 2);
    let result = base::capture(&default).preview_ace16_insert_rows(
        "Items",
        &[null(4).request()],
        &mut base::budget(),
        &|| false,
    );
    assert_eq!(
        result.err(),
        Some(EditError::Unsupported(WriteBlocker::Properties))
    );
}

#[test]
fn reader_valid_root_residue_and_fresh_padding_forgeries_fail_final_request_proof() {
    use super::super::{compose, finish, preflight, prepare};
    let case = model::source(6, Shape::PrefixLoss, Maps::Inline, true);
    let rows = [null(i32::MAX)];
    let expected = run(&case, &rows);
    for mutation in 0..2 {
        let requests = rows.iter().map(Row::request).collect::<Vec<_>>();
        let image = base::capture(&case.before);
        let before = preflight(&image, "Items", &requests, &mut base::budget(), &|| false).unwrap();
        let mut staged =
            prepare(&image, &before, &requests, &mut base::budget(), &|| false).unwrap();
        if mutation == 0 {
            let root = staged.indexes[0]
                .patches
                .iter_mut()
                .find(|patch| patch.offset == 13 * PAGE)
                .unwrap();
            assert_eq!(root.before, case.before[13 * PAGE..14 * PAGE]);
            assert_eq!(root.after, expected.bytes[13 * PAGE..14 * PAGE]);
            // Preserve canonical patch order: change only original-root residue.
            root.after[PAGE - 1] ^= 1;
        } else {
            let page = staged.indexes[0]
                .added
                .iter_mut()
                .find(|page| page.number == expected.fresh[0])
                .unwrap();
            page.bytes[PAGE - 1] ^= 1;
        }
        let forged = compose::compose(&image, &staged, &mut base::budget(), &|| false).unwrap();
        checked(&case, &forged, &rows);
        let result = finish(
            image,
            "Items",
            &requests,
            &staged,
            &mut base::budget(),
            &|| false,
        );
        assert_eq!(result.err(), Some(EditError::Preservation));
    }
}
