//! Local Native17 mechanics; these do not qualify the separate Source18 corpus.
use super::*;
use std::{
    io::Cursor,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_ace::NativeImage;
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime};
use tablecraft_expressions::{EvalError, functions::dates::clock::ClockReading};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_creation_defaults as fixtures;

const SCOPE: CatalogScope = CatalogScope::new(277);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn inserter(bytes: &[u8]) -> NativeJournalInserter {
    NativeJournalInserter::new(image(bytes), "Items", &mut budget(), &|| false).unwrap()
}

struct Provider {
    calls: AtomicUsize,
    available: bool,
}

impl Provider {
    fn new(available: bool) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            available,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}

impl Clock for Provider {
    fn read(&self) -> Result<ClockReading, EvalError> {
        if self.calls.fetch_add(1, Ordering::Relaxed) != 0 || !self.available {
            return Err(EvalError::ClockUnavailable);
        }
        ClockReading::new(ExtendedDateTime::new(1899, 12, 30, 12, 0, 0, 0).unwrap())
    }
}

fn omitted(id: i32) -> Vec<InsertValue> {
    vec![
        InsertValue::Supplied(Value::Int32(id)),
        InsertValue::Omitted,
    ]
}

fn supplied(id: i32, date_bits: Option<u64>) -> Vec<InsertValue> {
    vec![
        InsertValue::Supplied(Value::Int32(id)),
        InsertValue::Supplied(date_bits.map_or(Value::Null, |bits| {
            Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
        })),
    ]
}

fn prove(plan: NativeJournalInsertPlan, before: &[u8], after: &[u8], count: usize) {
    assert_eq!(plan.inserted_rows(), count);
    assert_eq!(plan.before_bytes(), before);
    assert!(
        plan.after_bytes() == after,
        "independent complete AFTER differs"
    );
    let fresh = plan
        .requalify_with_budgets(image(before), &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(fresh.inserted_rows(), count);
    assert!(
        fresh.after_bytes() == after,
        "fresh native proof changed AFTER"
    );
}

#[test]
fn omitted_clock_batch_captures_one_reading_and_preserves_each_explicit_null() {
    for mixed_null in [false, true] {
        let before = fixtures::clock_source();
        let provider = Provider::new(true);
        let rows = vec![
            omitted(24),
            if mixed_null {
                supplied(25, None)
            } else {
                omitted(25)
            },
        ];
        let expected = fixtures::clock_inserted_rows(
            &before,
            &[
                (24, Some(0.5_f64.to_bits())),
                (
                    25,
                    if mixed_null {
                        None
                    } else {
                        Some(0.5_f64.to_bits())
                    },
                ),
            ],
        );
        let plan = inserter(&before)
            .with_clock(&provider)
            .plan_insert_rows_with_omissions(rows, &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(plan, NativeJournalInsertPlan::Clock(_)));
        assert_eq!(provider.calls(), 1);
        prove(plan, &before, &expected, 2);
        assert_eq!(provider.calls(), 1, "fresh proof must use retained input");
    }
}

#[test]
fn complete_and_supplied_presence_batches_never_read_an_unused_clock() {
    let before = fixtures::clock_source();
    let expected =
        fixtures::clock_inserted_rows(&before, &[(24, Some(0.25_f64.to_bits())), (25, None)]);
    let provider = Provider::new(false);
    let plan = inserter(&before)
        .with_clock(&provider)
        .plan_insert_rows(
            vec![
                vec![
                    Value::Int32(24),
                    Value::DateTime(LegacyDateTime::from_bits(0.25_f64.to_bits()).unwrap()),
                ],
                vec![Value::Int32(25), Value::Null],
            ],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(matches!(plan, NativeJournalInsertPlan::CompleteTyped(_)));
    prove(plan, &before, &expected, 2);
    assert_eq!(provider.calls(), 0);

    let provider = Provider::new(false);
    let plan = inserter(&before)
        .with_clock(&provider)
        .plan_insert_rows_with_omissions(
            vec![supplied(24, Some(0.25_f64.to_bits())), supplied(25, None)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(matches!(plan, NativeJournalInsertPlan::Omitted(_)));
    prove(plan, &before, &expected, 2);
    assert_eq!(provider.calls(), 0);
}

#[test]
fn invalid_later_rows_and_absent_clock_refuse_the_entire_batch_before_a_read() {
    let before = fixtures::clock_source();
    for invalid in [
        vec![InsertValue::Supplied(Value::Int32(25))],
        vec![
            InsertValue::Supplied(Value::Int32(25)),
            InsertValue::Supplied(Value::Int32(7)),
        ],
    ] {
        let provider = Provider::new(true);
        assert!(
            inserter(&before)
                .with_clock(&provider)
                .plan_insert_rows_with_omissions(vec![omitted(24), invalid], &mut budget(), &|| {
                    false
                })
                .is_err()
        );
        assert_eq!(provider.calls(), 0);
    }
    assert!(
        inserter(&before)
            .plan_insert_rows_with_omissions(vec![omitted(24), omitted(25)], &mut budget(), &|| {
                false
            })
            .is_err()
    );

    let provider = Provider::new(true);
    assert!(
        inserter(&before)
            .with_clock(&provider)
            .plan_insert_rows_with_omissions(vec![omitted(24)], &mut budget(), &|| true)
            .is_err()
    );
    assert_eq!(provider.calls(), 0);
}

#[test]
fn empty_batches_still_qualify_the_source_without_sampling_a_clock() {
    let before = fixtures::clock_source();
    let provider = Provider::new(false);
    let plan = inserter(&before)
        .with_clock(&provider)
        .plan_insert_rows_with_omissions(Vec::new(), &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(plan, NativeJournalInsertPlan::Omitted(_)));
    prove(plan, &before, &before, 0);
    assert_eq!(provider.calls(), 0);

    let provider = Provider::new(false);
    let plan = inserter(&before)
        .with_clock(&provider)
        .plan_insert_rows(Vec::new(), &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(plan, NativeJournalInsertPlan::CompleteTyped(_)));
    prove(plan, &before, &before, 0);
    assert_eq!(provider.calls(), 0);
}
