use super::{DeleteError, plan_delete};
use std::{cell::Cell, rc::Rc};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{DeletableTable, RowHandle, Snapshot, SnapshotError};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};
use tablecraft_sql::{ParseLimits, parse_query};

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 100_000, 1_000_000)
}

struct Deleter {
    snapshot: Snapshot,
    calls: Rc<Cell<usize>>,
}

impl DeletableTable for Deleter {
    type Error = SnapshotError;
    type Plan = Vec<usize>;

    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    fn plan_deletes(
        self,
        rows: &[RowHandle],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        rows.iter().map(|&row| self.snapshot.ordinal(row)).collect()
    }
}

fn source(empty: bool) -> (Deleter, Rc<Cell<usize>>) {
    let columns = vec![
        Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
        Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
    ];
    let rows = if empty {
        Vec::new()
    } else {
        vec![
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(2), Value::Int32(20)],
            vec![Value::Int32(3), Value::Null],
        ]
    };
    let snapshot = Snapshot::new(
        "Items".into(),
        columns,
        rows,
        SchemaPolicy {
            max_name_bytes: 255,
            max_columns: 256,
            names: NameComparison::AsciiInsensitive,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let calls = Rc::new(Cell::new(0));
    (
        Deleter {
            snapshot,
            calls: calls.clone(),
        },
        calls,
    )
}

#[test]
fn delete_alias_targets_three_valued_predicates_and_no_match_finish_once() {
    for (sql, expected) in [
        ("DELETE FROM Items WHERE Id BETWEEN 1 AND 2", vec![0, 1]),
        ("DELETE i.* FROM Items AS i WHERE i.Id IN (1,3)", vec![0, 2]),
        ("DELETE * FROM Items WHERE Score IS NULL", vec![2]),
        ("DELETE FROM Items WHERE Score=20", vec![1]),
        ("DELETE FROM Items WHERE Abs(Id)>0", vec![0, 1, 2]),
        ("DELETE FROM Items WHERE NULL", vec![]),
        ("DELETE FROM Items WHERE FALSE", vec![]),
        ("DELETE FROM Items", vec![0, 1, 2]),
    ] {
        let (source, calls) = source(false);
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = plan_delete(source, query.query(), &mut budget(), None, &|| false).unwrap();
        assert_eq!(plan.matched_rows(), expected.len());
        assert_eq!(plan.into_backend(), expected);
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn delete_closed_binding_and_late_eager_errors_never_finish() {
    for sql in [
        "DELETE wrong.* FROM Items",
        "DELETE FROM Items WHERE Missing=1",
        "DELETE FROM Items WHERE Id IN (SELECT Id FROM Items)",
        "DELETE FROM Items WHERE Rnd()>0",
        "DELETE FROM Items WHERE DCount('*','Hidden')>0",
        "DELETE FROM Items WHERE 10/(2-Id)>0",
        "DELETE FROM Items WHERE FALSE AND 1/0>0",
        "UPDATE Items SET Score=1",
    ] {
        let (source, calls) = source(false);
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
            plan_delete(source, query.query(), &mut budget(), None, &|| false).is_err(),
            "{sql}"
        );
        assert_eq!(calls.get(), 0, "{sql}");
    }
    let (source, calls) = source(true);
    let query = parse_query("DELETE FROM Items WHERE Missing=1", ParseLimits::default()).unwrap();
    assert!(plan_delete(source, query.query(), &mut budget(), None, &|| false).is_err());
    assert_eq!(calls.get(), 0);
}

#[test]
fn delete_cancellation_and_limits_precede_backend_finish() {
    let query = parse_query("DELETE FROM Items", ParseLimits::default()).unwrap();
    let (initial, calls) = source(false);
    let polls = Cell::new(0);
    plan_delete(initial, query.query(), &mut budget(), None, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    let final_poll = polls.get();
    let (cancelled, calls) = source(false);
    let polls = Cell::new(0);
    assert!(matches!(
        plan_delete(cancelled, query.query(), &mut budget(), None, &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(DeleteError::Query(crate::QueryError::Cancelled))
    ));
    assert_eq!(calls.get(), 0);
    for mut limit in [
        ResourceBudget::new(0, 100, 100_000),
        ResourceBudget::new(32 * 1024 * 1024, 0, 100_000),
    ] {
        let (source, calls) = source(false);
        assert!(plan_delete(source, query.query(), &mut limit, None, &|| false).is_err());
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn delete_selection_capacity_exact_and_one_below_bounds_precede_finish() {
    let query = parse_query("DELETE FROM Items", ParseLimits::default()).unwrap();
    let (initial, calls) = source(false);
    let mut measured = budget();
    let available = measured.remaining(Resource::Bytes);

    let plan = plan_delete(initial, query.query(), &mut measured, None, &|| false).unwrap();
    assert_eq!(plan.into_backend(), [0, 1, 2]);
    assert_eq!(calls.get(), 1);
    let required = available - measured.remaining(Resource::Bytes);

    let (exact, calls) = source(false);
    let mut allowance = ResourceBudget::new(required, 100_000, 1_000_000);
    let polls = Cell::new(0);
    let plan = plan_delete(exact, query.query(), &mut allowance, None, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(plan.into_backend(), [0, 1, 2]);
    assert_eq!(allowance.remaining(Resource::Bytes), 0);
    assert_eq!(calls.get(), 1);

    let (denied, calls) = source(false);
    let mut allowance = ResourceBudget::new(required - 1, 100_000, 1_000_000);
    assert!(plan_delete(denied, query.query(), &mut allowance, None, &|| false).is_err());
    assert_eq!(calls.get(), 0);

    let final_poll = polls.get();
    let (cancelled, calls) = source(false);
    let mut allowance = ResourceBudget::new(required, 100_000, 1_000_000);
    let polls = Cell::new(0);
    assert!(matches!(
        plan_delete(cancelled, query.query(), &mut allowance, None, &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(DeleteError::Query(crate::QueryError::Cancelled))
    ));
    assert_eq!(calls.get(), 0);
}
