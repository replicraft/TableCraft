//! Independent logical-root tombstones over retained schemas and current inserts.
use super::*;
use crate::read_table;
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::{
    self as model,
    mixed::after_off::{
        current_delete as raw,
        deleted_field::populated::{CASES, Case, readdition::Target},
    },
};

const SCOPE: CatalogScope = CatalogScope::new(2802);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 40_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
    );
}

#[test]
fn current_delete_preserves_physical_payloads_and_retained_generated_history() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                for renamed in [false, true] {
                    for inserted in [false, true] {
                        let source = raw::source(case, target, filled, renamed, inserted);
                        let last = source.rows.len() - 1;
                        for ordinals in [vec![0], vec![last], (0..source.rows.len()).collect()] {
                            let native = image(&source.bytes);
                            assert!(
                                native
                                    .select_native_current_delete_source(
                                        "Table1",
                                        &mut budget(),
                                        &|| false,
                                    )
                                    .unwrap()
                            );
                            let requests = native
                                .prepare_current_row_deletes(
                                    "Table1",
                                    &ordinals,
                                    &mut budget(),
                                    &|| false,
                                )
                                .unwrap_or_else(|error| {
                                    panic!(
                                        "request {case:?} {target:?} filled={filled} renamed={renamed} inserted={inserted} {ordinals:?}: {error:?}"
                                    )
                                });
                            let roots: Vec<_> = ordinals
                                .iter()
                                .map(|&ordinal| source.roots[ordinal])
                                .collect();
                            let expected = raw::after(&source.bytes, &roots);
                            let plan = native
                                .plan_current_row_deletes(
                                    "Table1",
                                    &requests,
                                    &mut budget(),
                                    &|| false,
                                )
                                .unwrap_or_else(|error| {
                                    panic!(
                                        "plan {case:?} {target:?} filled={filled} renamed={renamed} inserted={inserted} {ordinals:?}: {error:?}"
                                    )
                                });
                            equal_image(plan.before_bytes(), &source.bytes);
                            equal_image(plan.after_bytes(), &expected);
                            assert_eq!(plan.changed_rows(), ordinals.len());

                            let table = read_table(
                                &mut Cursor::new(plan.after_bytes()),
                                SCOPE,
                                "Table1",
                                &mut budget(),
                                &|| false,
                            )
                            .unwrap();
                            assert!(
                                table
                                    .rows()
                                    .iter()
                                    .map(Vec::as_slice)
                                    .eq(source.survivors(&roots))
                            );
                            let final_image = image(plan.after_bytes());
                            assert!(
                                final_image
                                    .prepare_current_row_deletes(
                                        "Table1",
                                        &[],
                                        &mut budget(),
                                        &|| false,
                                    )
                                    .unwrap()
                                    .is_empty()
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn current_delete_rechecks_original_requests_and_exact_after_complements() {
    let source = raw::source(Case::TextPresent, Target::ShortText, true, true, true);
    let native = image(&source.bytes);
    assert!(
        native
            .prepare_ordinary_row_deletes("Table1", &[], &mut budget(), &|| false)
            .is_err()
    );
    let dense = model::source(model::Source::GuiRequiredZero);
    assert!(
        image(&dense)
            .prepare_current_row_deletes("Table1", &[], &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        native
            .prepare_current_row_deletes("Table1", &[0, 0], &mut budget(), &|| false)
            .is_err()
    );
    assert!(matches!(
        native
            .prepare_current_row_deletes("Table1", &[source.rows.len()], &mut budget(), &|| false,),
        Err(EditError::Conflict)
    ));
    assert!(
        native
            .prepare_current_row_deletes("Table1", &[], &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        native
            .prepare_current_row_deletes("Table1", &[], &mut ResourceBudget::new(0, 0, 0), &|| {
                false
            },)
            .is_err()
    );
    let empty = image(&source.bytes)
        .plan_current_row_deletes("Table1", &[], &mut budget(), &|| false)
        .unwrap();
    equal_image(empty.after_bytes(), &source.bytes);

    let requests = native
        .prepare_current_row_deletes("Table1", &[0], &mut budget(), &|| false)
        .unwrap();
    let request = &requests[0];
    let mut stale = request.expected_bytes().to_vec();
    stale[0] ^= 1;
    assert!(matches!(
        image(&source.bytes).plan_current_row_deletes(
            "Table1",
            &[RowDelete::new(0, stale)],
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Conflict)
    ));
    let mut plan = image(&source.bytes)
        .plan_current_row_deletes("Table1", &requests, &mut budget(), &|| false)
        .unwrap();
    plan.after[8 * 4096 + 4000] ^= 1;
    assert!(
        super::super::proof::prove_with_source(
            &plan,
            SCOPE,
            "Table1",
            &requests,
            Family::Current,
            &mut budget(),
            &|| false,
        )
        .is_err()
    );
}
