use super::support::*;
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare};

struct FailingComparison<'a> {
    active: &'a Cell<bool>,
    calls: Cell<usize>,
}

impl Collation for FailingComparison<'_> {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        assert!(
            !self.active.get(),
            "ON callbacks must run after source cursors finish"
        );
        self.calls.set(self.calls.get() + 1);
        Err(EvalError::CollationFailed)
    }
}

#[test]
fn complete_source_validation_and_eager_on_errors_precede_filters_or_top() {
    let data = Data::fixture();
    let sql = "SELECT TOP 1 L.A,R.A FROM L INNER JOIN R ON L.Id=R.Id AND 1/(R.K-7)>0 WHERE False";
    assert_eq!(
        execute(sql, &data, false).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(*data.scans.borrow(), ["L", "R"]);

    let data = Data::fixture();
    let provider = FailingComparison {
        active: &data.active,
        calls: Cell::new(0),
    };
    let plan = prepare(
        &syntax("SELECT L.A,R.A FROM L INNER JOIN R ON CStr(L.Id)=CStr(R.Id)"),
        &data,
    )
    .unwrap();
    let result = plan.execute_collections(
        &data,
        &[],
        ExecutionOptions {
            number_format: Some(NumberFormat::Invariant),
            collation: Some(&provider),
            ..Default::default()
        },
    );
    assert_eq!(
        result.err(),
        Some(QueryError::Evaluation(EvalError::CollationFailed))
    );
    assert_eq!(provider.calls.get(), 1);
    assert_eq!(*data.scans.borrow(), ["L", "R"]);

    let mut data = Data::fixture();
    data.tables[0].rows.clear();
    data.fail_table.set(Some("R"));
    let result = execute(
        "SELECT L.A,R.A FROM L INNER JOIN R ON L.Id=R.Id",
        &data,
        false,
    );
    assert_eq!(result.err(), Some(QueryError::Source));
    assert_eq!(
        *data.scans.borrow(),
        ["L", "R"],
        "an empty left side cannot hide a right source failure"
    );
}

#[test]
fn unsupported_collection_shapes_refuse_before_collection_cursors_even_with_false_filters() {
    for sql in [
        "SELECT L.A,R.A.Value FROM L,R WHERE False",
        "SELECT DISTINCT L.A FROM L INNER JOIN R ON L.Id=R.Id WHERE False",
        "SELECT L.A FROM L INNER JOIN R ON L.A=R.A WHERE False",
    ] {
        let data = Data::fixture();
        assert_eq!(
            execute(sql, &data, false).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
        assert!(data.scans.borrow().is_empty(), "{sql}");
    }
}

#[test]
fn shared_quotas_every_observed_cancel_checkpoint_and_plan_reuse_keep_results_private() {
    let data = Data::fixture();
    let query = syntax("SELECT L.A,R.A,L.Id FROM L INNER JOIN R ON L.Id=R.Id");
    let plan = prepare(&query, &data).unwrap();
    let checks = Cell::new(0);
    let observe = || {
        checks.set(checks.get() + 1);
        false
    };
    let expected = plan
        .execute_collections(
            &data,
            &[],
            ExecutionOptions {
                cancelled: Some(&observe),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(expected.rows().len(), 4);
    for stop in 1..=checks.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() >= stop
        };
        let result = plan.execute_collections(
            &data,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
            },
        );
        assert!(
            matches!(
                result.err(),
                Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
        assert!(!data.active.get());
    }
    for limits in [
        QueryLimits {
            bytes: 0,
            ..Default::default()
        },
        QueryLimits {
            rows: 0,
            ..Default::default()
        },
        QueryLimits {
            steps: 0,
            ..Default::default()
        },
    ] {
        let result = plan.execute_collections(
            &data,
            &[],
            ExecutionOptions {
                limits,
                ..Default::default()
            },
        );
        assert!(matches!(result.err(), Some(QueryError::Limit(_))));
    }
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_eq!(result.rows(), expected.rows());
}

#[test]
fn swallowed_visitor_failures_and_late_source_errors_do_not_publish_or_cache_prefixes() {
    let mut data = Data::fixture();
    data.tables[0].rows[0].lists[1] = vec![Value::Text("x".repeat(16384))];
    data.swallow.set(true);
    let plan = prepare(
        &syntax("SELECT L.B,R.A FROM L INNER JOIN R ON L.Id=R.Id"),
        &data,
    )
    .unwrap();
    let result = plan.execute_collections(
        &data,
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                bytes: 2048,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(
        matches!(result.err(), Some(QueryError::Limit(error)) if error.resource == Resource::Bytes)
    );
    assert!(
        data.rejected.get() > 0,
        "the source must actually swallow a rejected callback"
    );
    data.swallow.set(false);
    data.scans.borrow_mut().clear();
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_eq!(result.rows().len(), 4);
    assert_eq!(*data.scans.borrow(), ["L", "R"]);

    data.fail_table.set(Some("R"));
    assert_eq!(
        plan.execute_collections(&data, &[], Default::default())
            .err(),
        Some(QueryError::Source)
    );
    data.fail_table.set(None);
    data.scans.borrow_mut().clear();
    assert_eq!(
        plan.execute_collections(&data, &[], Default::default())
            .unwrap()
            .rows()
            .len(),
        4
    );
    assert_eq!(*data.scans.borrow(), ["L", "R"]);
}
