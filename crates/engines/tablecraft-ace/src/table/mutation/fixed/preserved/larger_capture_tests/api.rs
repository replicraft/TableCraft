//! Existing fixed APIs over immutable extended sources; no fixture writer authority.
use super::{budget, capture, corrected, model, small_image};
use crate::{
    Ace16WritePreview, EditError, FixedColumnUpdates, FixedScalarUpdate, FixedUpdateImageLimits,
    IndexCheckStatus, IndexConstraintViolation, InspectError, NativeImage, WriteBlocker,
    check_table, check_table_constraints,
    table::mutation::{gates, variable_split::metadata::Metadata},
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

fn same_bytes(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let first = actual.iter().zip(expected).position(|(a, b)| a != b);
    assert!(first.is_none(), "first differing byte {first:?}");
}

fn requests(preserved: bool, noop: bool) -> (&'static str, Vec<FixedScalarUpdate>) {
    if preserved {
        (
            "Yes_Field",
            vec![FixedScalarUpdate::new(
                0,
                Value::Boolean(false),
                Value::Boolean(!noop),
            )],
        )
    } else {
        (
            "Score",
            vec![FixedScalarUpdate::new(
                0,
                Value::Int32(10),
                Value::Int32(if noop { 10 } else { 11 }),
            )],
        )
    }
}

fn preview(
    bytes: &[u8],
    column: &str,
    updates: &[FixedScalarUpdate],
    batch: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let image = NativeImage::capture_for_fixed_updates(
        &mut Cursor::new(bytes),
        super::SCOPE,
        FixedUpdateImageLimits::new(model::EXPLICIT_BYTES as u64)?,
        budget,
        cancelled,
    )?;
    if batch {
        image.preview_ace16_batch_updates(
            &[FixedColumnUpdates::new("Items", column, updates)],
            budget,
            cancelled,
        )
    } else {
        image.preview_ace16_fixed_updates("Items", column, updates, budget, cancelled)
    }
}

fn small_pair() -> (Vec<u8>, Vec<u8>) {
    // Independent source derivative for repeated pool controls: retain the exact
    // low source and GLOBAL bitmap32; drop only the already allocated opaque tail.
    let case = corrected::case(6, model::Shape::ScalarDefault);
    let mut before = case.before;
    let mut expected = case.expected;
    before.truncate(33 * model::PAGE);
    expected.truncate(33 * model::PAGE);
    (before, expected)
}

fn alias_source() -> Vec<u8> {
    let (mut before, _) = small_pair();
    let row = model::row_span(&before, 15, 0);
    assert_eq!(before[row.start + 5 + 12 / 8], 1 << (13 % 8));
    before[row.start + 5 + 12 / 8] |= 1 << (12 % 8);
    before
}

#[test]
fn qualify_small_resource_pair_and_reader_valid_body_alias_before_extended_fixed_writer() {
    let (before, expected) = small_pair();
    for bytes in [&before, &expected] {
        let image = capture(bytes, model::EXPLICIT_BYTES as u64).unwrap();
        let checked =
            gates::before_fixed_update(&image, "Items", &mut budget(), &|| false).unwrap();
        assert_eq!(checked.rows(), 3);
        let metadata = Metadata::capture_for_fitted_updates(
            &image,
            checked.schema(),
            checked.indexes(),
            &[1],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        metadata
            .require_data_page(12, &mut budget(), &|| false)
            .unwrap();
        check_table_constraints(
            &mut image.reader(),
            super::SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
    assert_eq!(before.len(), 33 * model::PAGE);
    let alias = alias_source();
    let checked = check_table(
        &mut Cursor::new(&alias),
        super::SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.indexes().len(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let image = capture(&alias, model::EXPLICIT_BYTES as u64).unwrap();
    let metadata = Metadata::capture_for_fitted_updates(
        &image,
        checked.schema(),
        checked.indexes(),
        &[1],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        metadata.require_data_page(12, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}

#[test]
fn extended_fixed_single_and_batch_match_twenty_prefrozen_images_and_noops() {
    let mut comparisons = 0;
    for code in [5, 6] {
        for shape in model::Shape::ALL {
            if code == 5 && shape.preserved() {
                continue;
            }
            let case = corrected::case(code, shape);
            let (column, updates) = requests(shape.preserved(), false);
            let (_, noops) = requests(shape.preserved(), true);
            for batch in [false, true] {
                let candidate = preview(
                    &case.before,
                    column,
                    &updates,
                    batch,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(candidate.changed_rows(), 1);
                same_bytes(candidate.before_bytes(), &case.before);
                same_bytes(candidate.candidate_bytes(), &case.expected);
                comparisons += 1;
                drop(candidate);
                let candidate =
                    preview(&case.before, column, &noops, batch, &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(candidate.changed_rows(), 0);
                same_bytes(candidate.candidate_bytes(), &case.before);
            }
        }
    }
    assert_eq!(comparisons, 20);
}

#[test]
fn header5_rich_sources_keep_typed_refusals_and_jet4_remains_ineligible() {
    for shape in [model::Shape::PreservedDefault, model::Shape::PreservedHigh] {
        let case = model::case(5, shape);
        let (column, updates) = requests(true, false);
        for batch in [false, true] {
            assert!(matches!(
                preview(
                    &case.before,
                    column,
                    &updates,
                    batch,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Unsupported(WriteBlocker::Column))
            ));
        }
    }
    assert!(matches!(
        small_image(1).preview_ace16_fixed_updates("Items", "Score", &[], &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Format))
    ));
    assert!(matches!(
        small_image(1).preview_ace16_batch_updates(&[], &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Format))
    ));
}

#[test]
fn fixed_candidate_recapture_retains_original_policy_ceiling_and_exact_length() {
    let (before, expected) = small_pair();
    let ceiling = before.len() as u64;
    let extended = capture(&before, ceiling).unwrap();
    let candidate = extended
        .recapture_fixed_candidate(&expected, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        candidate.fixed_update_limits().unwrap().max_bytes(),
        ceiling
    );
    assert!(matches!(
        candidate.require_standard_capture(),
        Err(InspectError::StructureLimit)
    ));
    same_bytes(candidate.as_bytes(), &expected);
    assert!(matches!(
        extended
            .recapture_fixed_candidate(&expected[..expected.len() - 1], &mut budget(), &|| false),
        Err(InspectError::Malformed)
    ));
    let mut oversized = expected.clone();
    oversized.push(0);
    assert!(matches!(
        extended.recapture_fixed_candidate(&oversized, &mut budget(), &|| false),
        Err(InspectError::Malformed)
    ));
    let standard = NativeImage::capture(
        &mut Cursor::new(&before),
        super::SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let candidate = standard
        .recapture_fixed_candidate(&expected, &mut budget(), &|| false)
        .unwrap();
    assert!(candidate.fixed_update_limits().is_none());
    assert!(candidate.require_standard_capture().is_ok());
}

#[test]
fn source_alias_and_late_intent_failures_return_no_extended_candidate() {
    let (before, _) = small_pair();
    let alias = alias_source();
    let (column, updates) = requests(false, false);
    for batch in [false, true] {
        assert!(matches!(
            preview(&alias, column, &updates, batch, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        ));
        assert!(matches!(
            preview(&alias, column, &[], batch, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        ));
        let stale = [
            FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(11)),
            FixedScalarUpdate::new(2, Value::Int32(999), Value::Int32(31)),
        ];
        assert!(matches!(
            preview(&before, column, &stale, batch, &mut budget(), &|| false),
            Err(EditError::Conflict)
        ));
        let repeated = [
            FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(11)),
            FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(12)),
        ];
        assert!(matches!(
            preview(&before, column, &repeated, batch, &mut budget(), &|| false),
            Err(EditError::Conflict)
        ));
        let duplicate = [FixedScalarUpdate::new(
            0,
            Value::Int32(10),
            Value::Int32(20),
        )];
        assert!(matches!(
            preview(&before, column, &duplicate, batch, &mut budget(), &|| false),
            Err(EditError::Inspect(InspectError::IndexConstraint(
                IndexConstraintViolation::DuplicateKey
            )))
        ));
    }
    // A completely empty group list names no target table: it returns an exact
    // no-op after capture/format qualification, without inventing table ownership.
    let image = capture(&before, model::EXPLICIT_BYTES as u64).unwrap();
    let noop = image
        .preview_ace16_batch_updates(&[], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(noop.changed_rows(), 0);
    same_bytes(noop.candidate_bytes(), &before);
}

#[test]
fn extended_fixed_operation_pays_exact_pools_and_final_cancellation() {
    let (before, expected) = small_pair();
    let (column, updates) = requests(false, false);
    for batch in [false, true] {
        let mut measured = budget();
        let polls = Cell::new(0);
        let candidate = preview(&before, column, &updates, batch, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        same_bytes(candidate.candidate_bytes(), &expected);
        drop(candidate);
        for (resource, available) in [
            (Resource::Bytes, super::WORK_BYTES),
            (Resource::Rows, super::WORK_ROWS),
            (Resource::Steps, super::WORK_STEPS),
        ] {
            let needed = available - measured.remaining(resource);
            assert!(needed > 0);
            for (quota, success) in [(needed, true), (needed - 1, false)] {
                let (bytes, rows, steps) = match resource {
                    Resource::Bytes => (quota, super::WORK_ROWS, super::WORK_STEPS),
                    Resource::Rows => (super::WORK_BYTES, quota, super::WORK_STEPS),
                    Resource::Steps => (super::WORK_BYTES, super::WORK_ROWS, quota),
                };
                let result = preview(
                    &before,
                    column,
                    &updates,
                    batch,
                    &mut ResourceBudget::new(bytes, rows, steps),
                    &|| false,
                );
                if success {
                    same_bytes(result.unwrap().candidate_bytes(), &expected);
                } else {
                    assert!(matches!(
                        result,
                        Err(EditError::Inspect(InspectError::Limit(_)))
                    ));
                }
            }
        }
        let stop = polls.get();
        let polls = Cell::new(0);
        assert!(matches!(
            preview(&before, column, &updates, batch, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == stop
            }),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
    }
}

// Private diagnostic only; not registered. After the new SOURCE witness passes,
// root may append this exact test inside the existing api owner for a finite probe.
#[test]
fn standard_fixed_body_alias_diagnostic_after_source_qualification() {
    let before = alias_source();
    let (column, updates) = requests(false, false);
    let image = NativeImage::capture(
        &mut Cursor::new(&before),
        super::SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    match image.preview_ace16_fixed_updates("Items", column, &updates, &mut budget(), &|| false) {
        Err(EditError::Unsupported(WriteBlocker::RowLayout)) => {}
        Err(error) => panic!("distinct Standard admission failure: {error}"),
        Ok(candidate) => panic!(
            "Standard fixed accepted reader-valid DATA/INDEX body overlap; changed rows {}",
            candidate.changed_rows()
        ),
    }
}
