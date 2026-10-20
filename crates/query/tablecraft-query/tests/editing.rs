use std::{cell::Cell, rc::Rc};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, Snapshot, SnapshotError, UpdateProfile};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{UpdateError, plan_update};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};
use tablecraft_sql::{ParseLimits, parse_query};

struct OrdinalCollation;
impl Collation for OrdinalCollation {
    fn compare(&self, left: &str, right: &str) -> Result<std::cmp::Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 1_000_000)
}
struct Editor {
    snapshot: Snapshot,
    calls: Rc<Cell<usize>>,
}
impl EditableTable for Editor {
    type Error = SnapshotError;
    type Plan = Vec<(usize, Value)>;
    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    fn update_profile(&self) -> UpdateProfile {
        UpdateProfile::ExistingInt32
    }
    fn plan_updates(
        self,
        _: &str,
        updates: &[CellUpdate],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        updates
            .iter()
            .map(|update| Ok((self.snapshot.ordinal(update.row())?, update.value().clone())))
            .collect()
    }
}
fn editor(empty: bool) -> (Editor, Rc<Cell<usize>>) {
    let columns = [
        ("Id", ScalarType::Int32, false),
        ("Score", ScalarType::Int32, true),
        ("Label", ScalarType::Text, false),
    ]
    .into_iter()
    .map(|(name, kind, nullable)| Column::new(name.into(), kind, nullable).unwrap())
    .collect();
    let rows = if empty {
        vec![]
    } else {
        vec![
            vec![Value::Int32(1), Value::Int32(10), Value::Text("A".into())],
            vec![Value::Int32(2), Value::Int32(20), Value::Text("a".into())],
            vec![Value::Int32(3), Value::Null, Value::Text("B".into())],
        ]
    };
    let policy = SchemaPolicy {
        max_name_bytes: 255,
        max_columns: 256,
        names: NameComparison::AsciiInsensitive,
    };
    let snapshot = Snapshot::new(
        "Items".into(),
        columns,
        rows,
        policy,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let calls = Rc::new(Cell::new(0));
    (
        Editor {
            snapshot,
            calls: calls.clone(),
        },
        calls,
    )
}

#[test]
fn update_binding_aliases_filters_and_assignment_narrowing_use_snapshot_handles() {
    let (editor, calls) = editor(false);
    let query = parse_query(
        "UPDATE Items AS i SET i.Score = i.Score + 2 WHERE i.Id BETWEEN 1 AND 2",
        ParseLimits::default(),
    )
    .unwrap();
    let planned = plan_update(editor, query.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(planned.matched_rows(), 2);
    assert_eq!(calls.get(), 1);
    assert_eq!(
        planned.into_backend(),
        [(0, Value::Int32(12)), (1, Value::Int32(22))]
    );
}

#[test]
fn null_where_and_in_membership_keep_three_valued_query_behavior() {
    for (sql, expected) in [
        (
            "UPDATE Items SET Score=7 WHERE Id IN (2, NULL)",
            vec![(1, Value::Int32(7))],
        ),
        ("UPDATE Items SET Score=7 WHERE NULL=1", vec![]),
        (
            "UPDATE Items SET Score=7 WHERE Score IS NULL",
            vec![(2, Value::Int32(7))],
        ),
    ] {
        let (editor, calls) = editor(false);
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = plan_update(editor, query.query(), &mut budget(), None, &|| false).unwrap();
        assert_eq!(plan.into_backend(), expected);
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn late_expression_and_eager_filter_errors_never_call_the_backend() {
    for sql in [
        r"UPDATE Items SET Score=10 \ (Id-2)",
        "UPDATE Items SET Score=7 WHERE Id=1 OR 1/0=0",
        "UPDATE Items SET Score=2147483648",
        "UPDATE Items SET Score=NULL",
    ] {
        let (editor, calls) = editor(false);
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(plan_update(editor, query.query(), &mut budget(), None, &|| false).is_err());
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn unsupported_shapes_and_unknown_fields_fail_even_for_empty_tables() {
    for sql in [
        "UPDATE Items SET Score=1,Id=2",
        "UPDATE Items SET Score=Abs(Score)",
        "UPDATE Items SET Score=IIf(True,1,2)",
        "UPDATE Items SET Score=Nz(Score,0)",
        "UPDATE Items SET Score=Len('x')",
        "UPDATE Items SET Score=Year(#2026-01-01#)",
        "UPDATE Items SET Score=CInt(1)",
        "UPDATE Items SET Score=CCur(1)",
        "UPDATE Items SET Score=CDbl(1)",
        "UPDATE Items SET Score=DatePart('d',#2026-01-01#)",
        "UPDATE Items SET Score=DateDiff('d',#2026-01-01#,#2026-01-02#)",
        "UPDATE Items SET Score=Day(DateAdd('d',1,#2026-01-01#))",
        "UPDATE Items SET Score=1 WHERE IsNull(Score)",
        "UPDATE Items SET Score=(SELECT 1)",
        "UPDATE Items SET Score=MissingField",
        "UPDATE Items SET Score=1 WHERE MissingField=0",
        "PARAMETERS P LONG; UPDATE Items SET Score=P",
        "UPDATE Items INNER JOIN Other ON Items.Id=Other.Id SET Items.Score=1",
        "DELETE FROM Items",
    ] {
        let (editor, calls) = editor(true);
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
            plan_update(editor, query.query(), &mut budget(), None, &|| false).is_err(),
            "{sql}"
        );
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn text_filters_require_an_explicit_collation() {
    let query = parse_query(
        "UPDATE Items SET Score=1 WHERE Label='A'",
        ParseLimits::default(),
    )
    .unwrap();
    let (first, calls) = editor(false);
    assert!(plan_update(first, query.query(), &mut budget(), None, &|| false).is_err());
    assert_eq!(calls.get(), 0);
    let (second, _) = editor(false);
    let plan = plan_update(
        second,
        query.query(),
        &mut budget(),
        Some(&OrdinalCollation),
        &|| false,
    )
    .unwrap();
    assert_eq!(plan.into_backend(), [(0, Value::Int32(1))]);
}

#[test]
fn cancellation_and_shared_budget_exhaustion_publish_no_backend_candidate() {
    let query = parse_query("UPDATE Items SET Score=1", ParseLimits::default()).unwrap();
    let (first, calls) = editor(false);
    assert!(matches!(
        plan_update(first, query.query(), &mut budget(), None, &|| true),
        Err(UpdateError::Query(tablecraft_query::QueryError::Cancelled))
    ));
    assert_eq!(calls.get(), 0);
    let (second, calls) = editor(false);
    assert!(
        plan_update(
            second,
            query.query(),
            &mut ResourceBudget::new(0, 0, 10),
            None,
            &|| false
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
    let (third, calls) = editor(false);
    let checks = Cell::new(0);
    let cancelled = || {
        checks.set(checks.get() + 1);
        checks.get() > 8
    };
    assert!(matches!(
        plan_update(third, query.query(), &mut budget(), None, &cancelled),
        Err(UpdateError::Query(tablecraft_query::QueryError::Cancelled))
    ));
    assert_eq!(calls.get(), 0);
    let (fourth, calls) = editor(false);
    assert!(
        plan_update(
            fourth,
            query.query(),
            &mut ResourceBudget::new(16 * 1024 * 1024, 1, 1_000_000),
            None,
            &|| false
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
}
