//! A retained candidate cannot exchange its original request or finish cancelled.
use super::*;
use crate::RowInsert;
use std::{cell::Cell, io::Cursor};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_creation_defaults as fixtures;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(before),
        CatalogScope::new(260),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn plan(before: &[u8], id: i32) -> RecoverableNativeCompleteInsertPlan {
    image(before)
        .plan_recoverable_ordinary_complete_row_inserts(
            "Items",
            vec![RowInsert::new(vec![Value::Int32(id), Value::Null])],
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

#[test]
fn substituted_original_insert_request_refuses_even_with_same_before_binding() {
    let source = fixtures::source(fixtures::Case::Absent);
    let mut reviewed = plan(&source.bytes, 24);
    reviewed.intent = plan(&source.bytes, 25).intent;
    assert!(matches!(
        reviewed.requalify_with_budgets(
            image(&source.bytes),
            &mut budget(),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
}

#[test]
fn entry_middle_and_terminal_cancellation_never_publish_requalified_insert() {
    let source = fixtures::source(fixtures::Case::Absent);
    let polls = Cell::new(0);
    plan(&source.bytes, 24)
        .requalify_with_budgets(image(&source.bytes), &mut budget(), &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    for stop in [1, polls.get() / 2, polls.get()] {
        let seen = Cell::new(0);
        assert!(
            plan(&source.bytes, 24)
                .requalify_with_budgets(image(&source.bytes), &mut budget(), &mut budget(), &|| {
                    seen.set(seen.get() + 1);
                    seen.get() >= stop
                })
                .is_err()
        );
    }
}
