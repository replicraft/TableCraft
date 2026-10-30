use super::model;
use super::model::NewRow;
use crate::{EditError, IndexConstraintViolation, InspectError, RowInsert, WriteBlocker};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::insert_seed as source_model;
use tablecraft_testkit::access::insert_seed::PAGE;

#[test]
fn passive_defaults_preserve_full_explicit_rows_and_empty_batches() {
    for code in [5, 6] {
        let before = source_model::source(code, false, false, Some(("DefaultValue", "0")));
        model::checked(&before, 3, 1);
        for rows in [
            Vec::new(),
            vec![NewRow {
                id: 10,
                binary: None,
            }],
        ] {
            let expected = model::expected(&before, &rows).0;
            model::checked(&expected, 3 + rows.len(), 1);
            let requests = rows.iter().map(NewRow::request).collect::<Vec<_>>();
            let candidate = model::capture(&before)
                .preview_ace16_insert_rows("Items", &requests, &mut model::budget(), &|| false)
                .unwrap();
            assert_eq!(candidate.candidate_bytes(), expected);
        }
    }
}

#[test]
fn generated_ids_and_incomplete_or_wrong_carriers_never_produce_a_candidate() {
    for code in [5, 6] {
        let before = source_model::source(code, false, false, None);
        model::checked(&before, 3, 1);
        for values in [
            vec![Value::Int32(10)],
            vec![Value::Int32(10), Value::Null, Value::Null],
            vec![Value::Int64(10), Value::Null],
            vec![Value::Null, Value::Null],
            vec![Value::Int32(10), Value::Text("uncoerced".into())],
            vec![Value::Int32(10), Value::Binary(vec![0; 256])],
        ] {
            let error = model::capture(&before)
                .preview_ace16_insert_rows(
                    "Items",
                    &[RowInsert::new(values)],
                    &mut model::budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert_eq!(error, EditError::Unsupported(WriteBlocker::Column));
        }

        let mut generated = before.clone();
        // Literal first-column flags: fixed/updatable/AutoNumber, retaining a
        // genuine Int32 field and its healthy existing primary entries.
        generated[10 * PAGE + 75 + 15] = 7;
        model::checked(&generated, 3, 1);
        let error = model::capture(&generated)
            .preview_ace16_insert_rows("Items", &[], &mut model::budget(), &|| false)
            .err()
            .unwrap();
        assert_eq!(error, EditError::Unsupported(WriteBlocker::Column));
    }
}

#[test]
fn existing_and_batch_duplicate_primary_values_keep_typed_unique_failures() {
    for code in [5, 6] {
        let before = source_model::source(code, true, false, None);
        model::checked(&before, 3, 2);
        for ids in [vec![1], vec![10, 10]] {
            let requests = ids
                .into_iter()
                .map(|id| NewRow { id, binary: None }.request())
                .collect::<Vec<_>>();
            let error = model::capture(&before)
                .preview_ace16_insert_rows("Items", &requests, &mut model::budget(), &|| false)
                .err()
                .unwrap();
            assert_eq!(
                error,
                EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey
                ))
            );
        }
    }
}

#[test]
fn shared_populated_map_roles_do_not_grant_insert_authority() {
    for code in [5, 6] {
        let mut before = source_model::source(code, false, false, None);
        // Both ordinary roles now name the same healthy, populated map row.
        // Current row reads remain valid; mutable role identity is unqualified.
        support::put32(&mut before, 10 * PAGE + 59, 11 << 8);
        model::checked(&before, 3, 1);
        for requests in [
            Vec::new(),
            vec![
                NewRow {
                    id: 10,
                    binary: None,
                }
                .request(),
            ],
        ] {
            let error = model::capture(&before)
                .preview_ace16_insert_rows("Items", &requests, &mut model::budget(), &|| false)
                .err()
                .unwrap();
            assert_eq!(error, EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
}

#[test]
fn shared_quotas_late_cancellation_and_retry_preserve_complete_images() {
    let before = source_model::source(6, true, true, None);
    model::checked(&before, 3, 2);
    let rows = [NewRow {
        id: 10,
        binary: Some(vec![0xa5; 255]),
    }];
    let requests = rows.iter().map(NewRow::request).collect::<Vec<_>>();
    let (expected, _) = model::expected(&before, &rows);
    for (mut allowance, resource) in [
        (
            ResourceBudget::new(0, 100_000, 100_000_000),
            Resource::Bytes,
        ),
        (
            ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
            Resource::Rows,
        ),
        (
            ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
            Resource::Steps,
        ),
    ] {
        let error = model::capture(&before)
            .preview_ace16_insert_rows("Items", &requests, &mut allowance, &|| false)
            .err()
            .unwrap();
        assert!(
            matches!(error, EditError::Inspect(InspectError::Limit(limit)) if limit.resource == resource)
        );
    }

    let calls = Cell::new(0);
    let candidate = model::capture(&before)
        .preview_ace16_insert_rows("Items", &requests, &mut model::budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), expected);
    assert!(calls.get() > 2);
    let terminal = calls.get() - 1;
    let current = Cell::new(0);
    let error = model::capture(&before)
        .preview_ace16_insert_rows("Items", &requests, &mut model::budget(), &|| {
            current.set(current.get() + 1);
            current.get() == terminal
        })
        .err()
        .unwrap();
    assert_eq!(error, EditError::Inspect(InspectError::Cancelled));
    let retry = model::capture(&before)
        .preview_ace16_insert_rows("Items", &requests, &mut model::budget(), &|| false)
        .unwrap();
    assert_eq!(retry.candidate_bytes(), expected);
}
