//! Explicit zero pairs gain new maps without borrowing old container storage.
pub(in crate::table::mutation::insert) mod model;
mod order;

use super::{Family, PAGE, checked, original};
use crate::{
    EditError, InspectError, LongValueCheckStatus, RowInsert, WriteBlocker, check_table_long_values,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_model::CatalogScope;

#[test]
fn literal_zero_pair_sources_and_initialized_expected_images_are_readable() {
    for code in [5, 6] {
        for (family, value) in model::values() {
            let case = model::case(code, family, value);
            checked(&case.before, &model::rows(family, None));
            let report = check_table_long_values(
                &mut Cursor::new(&case.before),
                CatalogScope::new(1),
                "Items",
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                report.columns()[0].status(),
                LongValueCheckStatus::UnknownDeclaration
            );
            assert_eq!(report.columns()[0].maps().unwrap().owned(), 0);
            assert_eq!(report.columns()[0].maps().unwrap().candidate(), 0);
            checked(&case.expected, &model::rows(family, Some(&case.value)));
            model::checked_maps(&case.expected);
        }
    }
}

#[test]
fn explicit_zero_pairs_initialize_before_single_and_chained_memo_ole_bodies() {
    for code in [5, 6] {
        for (family, value) in model::values() {
            let case = model::case(code, family, value);
            checked(&case.before, &model::rows(family, None));
            checked(&case.expected, &model::rows(family, Some(&case.value)));
            model::checked_maps(&case.expected);
            let image = original::capture(&case.before);
            let request = RowInsert::new(vec![
                Value::Int32(10),
                Value::Int32(100),
                case.value.clone(),
            ]);
            let candidate = image
                .preview_ace16_insert_rows("Items", &[request], &mut original::budget(), &|| false)
                .unwrap();
            let actual = candidate.candidate_bytes();
            assert_eq!(actual.len(), case.expected.len());
            let difference = actual.iter().zip(&case.expected).position(|(a, b)| a != b);
            assert!(
                difference.is_none(),
                "{code}/{family:?}: first difference {difference:?}"
            );
            model::checked_maps(actual);
        }
    }
}

#[test]
fn inline_only_requests_leave_zero_pairs_uninitialized() {
    for code in [5, 6] {
        let case = model::case(code, Family::Ole, Value::Binary(vec![0x92; 65]));
        let image = original::capture(&case.before);
        let value = Value::Binary(vec![]);
        let request = RowInsert::new(vec![Value::Int32(10), Value::Int32(100), value.clone()]);
        let candidate = image
            .preview_ace16_insert_rows("Items", &[request], &mut original::budget(), &|| false)
            .unwrap();
        let actual = candidate.candidate_bytes();

        assert_eq!(actual.len(), case.before.len());
        assert_eq!(&actual[10 * PAGE + 500..10 * PAGE + 508], &[0; 8]);
        assert_eq!(
            &actual[11 * PAGE..12 * PAGE],
            &case.before[11 * PAGE..12 * PAGE]
        );
        checked(actual, &model::rows(Family::Ole, Some(&value)));
    }
}

#[test]
fn zero_pairs_with_existing_external_values_cannot_claim_empty_ownership() {
    for code in [5, 6] {
        for (family, value) in model::values() {
            let case = model::case(code, family, value);
            let mut before = case.expected;
            before[10 * PAGE + 500..10 * PAGE + 508].fill(0);
            checked(&before, &model::rows(family, Some(&case.value)));
            let image = original::capture(&before);
            let request = RowInsert::new(vec![Value::Int32(11), Value::Int32(110), case.value]);
            let result = image.preview_ace16_insert_rows(
                "Items",
                &[request],
                &mut original::budget(),
                &|| false,
            );

            assert!(matches!(
                result,
                Err(EditError::Unsupported(WriteBlocker::RowLayout))
            ));
        }
    }
}

#[test]
fn initialized_chains_publish_no_candidate_at_each_resource_or_terminal_cancellation_boundary() {
    let case = model::case(6, Family::Ole, Value::Binary(vec![0x92; 8145]));
    checked(&case.before, &model::rows(Family::Ole, None));
    checked(&case.expected, &model::rows(Family::Ole, Some(&case.value)));
    model::checked_maps(&case.expected);
    let image = original::capture(&case.before);
    let requests = [RowInsert::new(vec![
        Value::Int32(10),
        Value::Int32(100),
        case.value,
    ])];
    let polls = Cell::new(0_usize);
    let mut healthy_budget = original::budget();
    let healthy = image
        .preview_ace16_insert_rows("Items", &requests, &mut healthy_budget, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(healthy.candidate_bytes(), case.expected);

    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let initial = original::budget().remaining(resource);
        let used = initial - healthy_budget.remaining(resource);
        assert!(used > 0);
        let mut limited = original::budget();
        limited.charge(resource, initial - (used - 1)).unwrap();
        let image = original::capture(&case.before);
        let result = image.preview_ace16_insert_rows("Items", &requests, &mut limited, &|| false);

        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource)
        );
    }

    let observed = Cell::new(0_usize);
    let image = original::capture(&case.before);
    let result =
        image.preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| {
            observed.set(observed.get() + 1);
            observed.get() == polls.get()
        });

    assert!(matches!(
        result,
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(observed.get(), polls.get());
}
