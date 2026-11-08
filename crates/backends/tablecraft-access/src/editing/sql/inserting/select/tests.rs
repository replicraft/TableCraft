//! Actual native source scans precede one sealed append under the same pool.
use super::*;
use crate::NativeSource;
use std::{
    cell::RefCell,
    io::Cursor,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_ace::NativeImage;
use tablecraft_core::values::ExtendedDateTime;
use tablecraft_expressions::{EvalError, functions::dates::clock::ClockReading};
use tablecraft_model::CatalogScope;
use tablecraft_query::InsertError;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_creation_defaults as fixtures;

const SCOPE: CatalogScope = CatalogScope::new(281);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn inserter(before: &[u8], native: &RefCell<ResourceBudget>) -> NativeJournalInserter {
    let mut work = native.borrow_mut();
    let image =
        NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut work, &|| false).unwrap();
    NativeJournalInserter::new(image, "Items", &mut work, &|| false).unwrap()
}

struct Provider(AtomicUsize);

impl Provider {
    fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    fn calls(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

impl Clock for Provider {
    fn read(&self) -> Result<ClockReading, EvalError> {
        if self.0.fetch_add(1, Ordering::Relaxed) != 0 {
            return Err(EvalError::ClockUnavailable);
        }
        ClockReading::new(ExtendedDateTime::new(1899, 12, 30, 12, 0, 0, 0).unwrap())
    }
}

#[test]
fn original_self_select_preserves_omissions_null_and_reordered_complete_fields() {
    for (sql, supplied_null) in [
        (
            "PARAMETERS added LONG; INSERT INTO Items(Id) SELECT Id+added FROM Items",
            false,
        ),
        (
            "PARAMETERS added LONG; INSERT INTO Items(Score,Id) SELECT NULL,Id+added FROM Items",
            true,
        ),
    ] {
        let fixture = fixtures::source(fixtures::Case::Seven);
        let (expected, _) = fixtures::inserted(&fixture, supplied_null).unwrap();
        let native = RefCell::new(budget());
        let target = inserter(&fixture.bytes, &native);
        let source = NativeSource::new_with_shared_budget(
            Cursor::new(&fixture.bytes),
            SCOPE,
            &native,
            &|| false,
        )
        .unwrap();
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
            super::super::local_insert_table(query.query(), true, &mut budget(), &|| false)
                .is_err()
        );
        let parameters = [("added", Value::Int32(if supplied_null { 24 } else { 23 }))];
        let before_scan = native.borrow().remaining(Resource::Steps);
        let candidate = prepare_native_insert_select(
            target,
            query.query(),
            &source,
            &mut budget(),
            &parameters,
            ExecutionOptions::default(),
            None,
        )
        .unwrap();
        assert!(native.borrow().remaining(Resource::Steps) < before_scan);
        drop(source);
        let plan = candidate.finish(&mut native.borrow_mut()).unwrap();
        assert_eq!(plan.inserted_rows(), 1);
        assert_eq!(plan.before_bytes(), fixture.bytes);
        assert!(
            plan.after_bytes() == expected,
            "independent native append AFTER differs"
        );
        assert_eq!(
            matches!(plan, NativeJournalInsertPlan::CompleteTyped(_)),
            supplied_null
        );
    }
}

#[test]
fn entire_original_union_stages_before_one_clock_read_and_one_batch_finish() {
    let before = fixtures::clock_source();
    let expected = fixtures::clock_inserted_rows(
        &before,
        &[(24, Some(0.5_f64.to_bits())), (25, Some(0.5_f64.to_bits()))],
    );
    let native = RefCell::new(budget());
    let target = inserter(&before, &native);
    let source =
        NativeSource::new_with_shared_budget(Cursor::new(&before), SCOPE, &native, &|| false)
            .unwrap();
    let query = parse_query(
        "INSERT INTO Items(Id) SELECT Id+23 FROM Items UNION ALL SELECT Id+24 FROM Items",
        ParseLimits::default(),
    )
    .unwrap();
    let provider = Provider::new();
    let candidate = prepare_native_insert_select(
        target,
        query.query(),
        &source,
        &mut budget(),
        &[],
        ExecutionOptions::default(),
        Some(&provider),
    )
    .unwrap();
    assert_eq!(provider.calls(), 0);
    drop(source);
    let plan = candidate.finish(&mut native.borrow_mut()).unwrap();
    assert!(matches!(plan, NativeJournalInsertPlan::Clock(_)));
    assert_eq!(plan.inserted_rows(), 2);
    assert_eq!(provider.calls(), 1);
    assert!(
        plan.after_bytes() == expected,
        "entire original UNION must append only once"
    );
}

#[test]
fn late_query_failure_cannot_produce_a_candidate_or_sample_omitted_defaults() {
    let before = fixtures::clock_source();
    let native = RefCell::new(budget());
    let target = inserter(&before, &native);
    let source =
        NativeSource::new_with_shared_budget(Cursor::new(&before), SCOPE, &native, &|| false)
            .unwrap();
    let query = parse_query(
        "INSERT INTO Items(Id) SELECT Id+23 FROM Items UNION ALL SELECT 1/0 FROM Items",
        ParseLimits::default(),
    )
    .unwrap();
    let provider = Provider::new();
    assert!(matches!(
        prepare_native_insert_select(
            target,
            query.query(),
            &source,
            &mut budget(),
            &[],
            ExecutionOptions::default(),
            Some(&provider)
        ),
        Err(RecoverableInsertError::Plan(InsertError::Query(
            QueryError::Evaluation(_)
        )))
    ));
    assert_eq!(provider.calls(), 0);
}

#[test]
fn destination_mapping_refuses_before_unreadable_source_data_is_touched() {
    let mut before = fixtures::clock_source();
    before[8 * 4096] = 0;
    let native = RefCell::new(budget());
    let target = inserter(&before, &native);
    let source =
        NativeSource::new_with_shared_budget(Cursor::new(&before), SCOPE, &native, &|| false)
            .unwrap();
    let query = parse_query(
        "INSERT INTO Items(Missing) SELECT Id FROM Items",
        ParseLimits::default(),
    )
    .unwrap();
    let available = native.borrow().remaining(Resource::Steps);
    assert!(matches!(
        prepare_native_insert_select(
            target,
            query.query(),
            &source,
            &mut budget(),
            &[],
            ExecutionOptions::default(),
            None
        ),
        Err(RecoverableInsertError::Plan(InsertError::Query(
            QueryError::UnknownName
        )))
    ));
    assert_eq!(native.borrow().remaining(Resource::Steps), available);
}

#[test]
fn empty_select_finishes_source_admission_without_clock_or_native_changes() {
    let before = fixtures::clock_source();
    let native = RefCell::new(budget());
    let target = inserter(&before, &native);
    let source =
        NativeSource::new_with_shared_budget(Cursor::new(&before), SCOPE, &native, &|| false)
            .unwrap();
    let query = parse_query(
        "INSERT INTO Items(Id) SELECT Id FROM Items WHERE FALSE",
        ParseLimits::default(),
    )
    .unwrap();
    let provider = Provider::new();
    let candidate = prepare_native_insert_select(
        target,
        query.query(),
        &source,
        &mut budget(),
        &[],
        ExecutionOptions::default(),
        Some(&provider),
    )
    .unwrap();
    assert_eq!(provider.calls(), 0);
    drop(source);
    let plan = candidate.finish(&mut native.borrow_mut()).unwrap();
    assert_eq!(plan.inserted_rows(), 0);
    assert!(matches!(plan, NativeJournalInsertPlan::Omitted(_)));
    assert_eq!(plan.after_bytes(), before);
    assert_eq!(provider.calls(), 0);
}

#[test]
fn local_select_preflight_is_separate_from_values_and_bounds_target_names() {
    let selected = parse_query(
        "INSERT INTO Items(Id) SELECT Id FROM Items",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        local_insert_select_table(selected.query(), &mut budget(), &|| false).unwrap(),
        "Items"
    );
    let values = parse_query("INSERT INTO Items(Id) VALUES(1)", ParseLimits::default()).unwrap();
    assert!(matches!(
        local_insert_select_table(values.query(), &mut budget(), &|| false),
        Err(QueryError::Unsupported)
    ));
    assert!(matches!(
        local_insert_select_table(selected.query(), &mut budget(), &|| true),
        Err(QueryError::Cancelled)
    ));
    let wide = parse_query(
        &format!("INSERT INTO [{}](Id) SELECT Id FROM Items", "x".repeat(256)),
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        local_insert_select_table(wide.query(), &mut budget(), &|| false),
        Err(QueryError::StructureLimit)
    ));
}
