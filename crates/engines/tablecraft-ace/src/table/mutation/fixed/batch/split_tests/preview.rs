//! Real allocation API comparisons against the prefrozen complete-image model.
use super::{budget, model};
use crate::{FixedColumnUpdates, FixedScalarUpdate, NativeImage};
use model::{Case, Shape};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

pub(super) fn assignments(case: &Case, component: usize) -> Vec<FixedScalarUpdate> {
    assert!(matches!(component, 1 | 2));
    case.original
        .iter()
        .zip(&case.final_rows)
        .enumerate()
        .map(|(row, (before, after))| {
            let (expected, value) = if component == 1 {
                (before.left, after.left)
            } else {
                (before.right, after.right)
            };
            FixedScalarUpdate::new(
                row,
                expected.map_or(Value::Null, Value::Int32),
                value.map_or(Value::Null, Value::Int32),
            )
        })
        .collect()
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8], label: &str) {
    assert_eq!(actual.len(), expected.len(), "{label}: complete length");
    let mut count = 0;
    let mut first = None;
    for (offset, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        if actual != expected {
            count += 1;
            first.get_or_insert((offset, actual, expected));
        }
    }
    assert_eq!(count, 0, "{label}: first differing byte {first:?}");
}

#[test]
fn nullable_split_batch_matches_all_eighty_prefrozen_complete_images() {
    for code in [5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                for forwarded in [false, true] {
                    for shape in [
                        Shape::Mixed,
                        Shape::Empty,
                        Shape::Branch,
                        Shape::Clear,
                        Shape::BranchClear,
                    ] {
                        let case = model::source(code, shape, descending, unique, forwarded);
                        let label = format!(
                            "header={code} descending={descending} unique={unique} forwarded={forwarded} shape={shape:?}"
                        );
                        let left = assignments(&case, 1);
                        let right = assignments(&case, 2);
                        let changed_fields = case
                            .original
                            .iter()
                            .zip(&case.final_rows)
                            .map(|(before, after)| {
                                usize::from(before.left != after.left)
                                    + usize::from(before.right != after.right)
                            })
                            .sum::<usize>();
                        assert_eq!(
                            changed_fields,
                            if matches!(shape, Shape::Empty) {
                                1000
                            } else {
                                800
                            }
                        );
                        let groups = [
                            FixedColumnUpdates::new("Items", "Group", &left),
                            FixedColumnUpdates::new("Items", "Score", &right),
                        ];
                        let result = capture(&case.before)
                            .preview_ace16_split_batch_updates(&groups, &mut budget(), &|| false)
                            .unwrap_or_else(|error| panic!("{label}: {error:?}"));
                        assert_image(result.before_bytes(), &case.before, &label);
                        assert_image(result.candidate_bytes(), &case.expected, &label);
                        assert_eq!(result.changed_rows(), 500, "{label}: unique logical rows");
                    }
                }
            }
        }
    }
}

#[test]
fn allocation_batch_retains_exact_fitting_batch_outputs_without_new_pages() {
    for code in [5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                for forwarded in [false, true] {
                    let case =
                        model::source(code, Shape::BranchClear, descending, unique, forwarded);
                    assert!(case.added.is_empty());
                    let left = assignments(&case, 1);
                    let right = assignments(&case, 2);
                    let groups = [
                        FixedColumnUpdates::new("Items", "Group", &left),
                        FixedColumnUpdates::new("Items", "Score", &right),
                    ];
                    let fitted = capture(&case.before)
                        .preview_ace16_batch_updates(&groups, &mut budget(), &|| false)
                        .unwrap();
                    let allocating = capture(&case.before)
                        .preview_ace16_split_batch_updates(&groups, &mut budget(), &|| false)
                        .unwrap();
                    let label = format!(
                        "fitted header={code} descending={descending} unique={unique} forwarded={forwarded}"
                    );
                    assert_image(fitted.candidate_bytes(), &case.expected, &label);
                    assert_image(
                        allocating.candidate_bytes(),
                        fitted.candidate_bytes(),
                        &label,
                    );
                    assert_eq!(allocating.candidate_bytes().len(), case.before.len());
                    assert_eq!(allocating.changed_rows(), fitted.changed_rows());
                }
            }
        }
    }
}
