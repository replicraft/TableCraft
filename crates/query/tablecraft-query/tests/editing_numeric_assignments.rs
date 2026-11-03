//! SQL assignment conversion stays above exact typed backend carriers.
use std::{cell::Cell, rc::Rc};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Value},
};
use tablecraft_db::{
    CellUpdate, EditableTable, InsertableTable, Snapshot, SnapshotError, UpdateProfile,
};
use tablecraft_expressions::EvalError;
use tablecraft_query::{InsertError, QueryError, UpdateError, plan_insert, plan_update};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};
use tablecraft_sql::{ParseLimits, parse_query};

struct Target {
    snapshot: Snapshot,
    calls: Rc<Cell<usize>>,
}

impl EditableTable for Target {
    type Error = SnapshotError;
    type Plan = Vec<Value>;

    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    fn update_profile(&self) -> UpdateProfile {
        UpdateProfile::StoredScalar
    }

    fn plan_updates(
        self,
        _: &str,
        updates: &[CellUpdate],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        Ok(updates
            .iter()
            .map(|update| update.value().clone())
            .collect())
    }
}

impl InsertableTable for Target {
    type Error = SnapshotError;
    type Plan = Vec<Value>;

    fn name(&self) -> &str {
        "Items"
    }

    fn columns(&self) -> &[Column] {
        self.snapshot.columns()
    }

    fn schema_policy(&self) -> SchemaPolicy {
        policy()
    }

    fn requires_field_list(&self) -> bool {
        false
    }

    fn plan_insert(
        self,
        values: Vec<Value>,
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        Ok(values)
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 1_000_000)
}

fn policy() -> SchemaPolicy {
    SchemaPolicy {
        max_name_bytes: 255,
        max_columns: 256,
        names: NameComparison::AsciiInsensitive,
    }
}

fn integral(kind: ScalarType, integer: i64) -> Value {
    match kind {
        ScalarType::Byte => Value::Byte(u8::try_from(integer).unwrap()),
        ScalarType::Int16 => Value::Int16(i16::try_from(integer).unwrap()),
        ScalarType::Int64 => Value::Int64(integer),
        ScalarType::Currency => Value::Currency(Currency::from_scaled(integer * 10_000)),
        _ => unreachable!(),
    }
}

fn target(kind: ScalarType, nullable: bool, input: Vec<Value>) -> (Target, Rc<Cell<usize>>) {
    let initial = match kind {
        ScalarType::Byte => Value::Byte(0),
        ScalarType::Int16 => Value::Int16(0),
        ScalarType::Int64 => Value::Int64(0),
        ScalarType::Currency => Value::Currency(Currency::from_scaled(0)),
        _ => unreachable!(),
    };
    let rows = input
        .into_iter()
        .map(|value| vec![initial.clone(), value])
        .collect();
    let snapshot = Snapshot::new(
        "Items".to_owned(),
        vec![
            Column::new("Target".to_owned(), kind, nullable).unwrap(),
            Column::new("Input".to_owned(), ScalarType::Int64, false).unwrap(),
        ],
        rows,
        policy(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let calls = Rc::new(Cell::new(0));
    (
        Target {
            snapshot,
            calls: calls.clone(),
        },
        calls,
    )
}

#[test]
fn update_and_insert_small_integer_expressions_reach_backend_as_exact_target_carriers() {
    for kind in [
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int64,
        ScalarType::Currency,
    ] {
        for (expression, integer) in [
            ("0", 0),
            ("7", 7),
            ("-7", -7),
            ("CByte(7)", 7),
            ("CLng(7)", 7),
            ("7 - 0", 7),
            ("CLngLng(7)", 7),
        ] {
            if kind == ScalarType::Byte && integer < 0 {
                continue;
            }
            let expected = integral(kind, integer);
            let (editor, calls) = target(kind, false, vec![Value::Int64(1)]);
            let sql = format!("UPDATE Items SET Target={expression}");
            let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
            let plan = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
            assert_eq!(plan.into_backend(), vec![expected.clone()]);
            assert_eq!(calls.get(), 1);

            // INSERT's closed constructor profile does not include CLngLng.
            // Its integer literals and arithmetic exercise Int64 inputs directly.
            if expression == "CLngLng(7)" {
                continue;
            }

            let (editor, calls) = target(kind, false, vec![]);
            let sql = format!("INSERT INTO Items VALUES ({expression},0)");
            let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
            let values = plan_insert(editor, parsed.query(), &mut budget(), &|| false).unwrap();
            assert_eq!(values, vec![expected, Value::Int64(0)]);
            assert_eq!(calls.get(), 1);
        }
    }
}

#[test]
fn nullable_assignment_remains_distinct_from_required_and_nonintegral_values() {
    for kind in [
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int64,
        ScalarType::Currency,
    ] {
        let (editor, calls) = target(kind, true, vec![Value::Int64(1)]);
        let parsed = parse_query("UPDATE Items SET Target=NULL", ParseLimits::default()).unwrap();
        let plan = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
        assert_eq!(plan.into_backend(), vec![Value::Null]);
        assert_eq!(calls.get(), 1);

        let (editor, calls) = target(kind, true, vec![]);
        let parsed =
            parse_query("INSERT INTO Items VALUES (NULL,0)", ParseLimits::default()).unwrap();
        assert_eq!(
            plan_insert(editor, parsed.query(), &mut budget(), &|| false).unwrap(),
            vec![Value::Null, Value::Int64(0)]
        );
        assert_eq!(calls.get(), 1);

        for expression in ["NULL", "True", "'7'", "CDbl(7)"] {
            let (editor, calls) = target(kind, false, vec![Value::Int64(1)]);
            let sql = format!("UPDATE Items SET Target={expression}");
            let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
            assert!(matches!(
                plan_update(editor, parsed.query(), &mut budget(), None, &|| false),
                Err(UpdateError::Query(QueryError::Unsupported))
            ));
            assert_eq!(calls.get(), 0);

            let (editor, calls) = target(kind, false, vec![]);
            let sql = format!("INSERT INTO Items VALUES ({expression},0)");
            let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
            assert!(matches!(
                plan_insert(editor, parsed.query(), &mut budget(), &|| false),
                Err(InsertError::Query(QueryError::Unsupported))
            ));
            assert_eq!(calls.get(), 0);
        }
    }
}

#[test]
fn late_currency_overflow_never_publishes_an_earlier_valid_assignment() {
    let (editor, calls) = target(
        ScalarType::Currency,
        false,
        vec![Value::Int64(1), Value::Int64(922_337_203_685_478)],
    );
    let parsed = parse_query("UPDATE Items SET Target=[Input]", ParseLimits::default()).unwrap();
    assert!(matches!(
        plan_update(editor, parsed.query(), &mut budget(), None, &|| false),
        Err(UpdateError::Query(QueryError::Evaluation(
            EvalError::Overflow
        )))
    ));
    assert_eq!(calls.get(), 0);

    let (editor, calls) = target(ScalarType::Currency, false, vec![]);
    let parsed = parse_query(
        "INSERT INTO Items VALUES (922337203685478,0)",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        plan_insert(editor, parsed.query(), &mut budget(), &|| false),
        Err(InsertError::Query(QueryError::Evaluation(
            EvalError::Overflow
        )))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn both_public_planners_enforce_exact_work_and_cancel_before_finish() {
    for (kind, insert) in [ScalarType::Byte, ScalarType::Int16, ScalarType::Currency]
        .into_iter()
        .flat_map(|kind| [false, true].map(|insert| (kind, insert)))
    {
        let sql = if insert {
            "INSERT INTO Items VALUES (0,0)"
        } else if kind == ScalarType::Int16 {
            // Arithmetic supplies Int64, exercising the new narrowing work.
            "UPDATE Items SET Target=0 - 0"
        } else {
            "UPDATE Items SET Target=0"
        };
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let run = |editor: Target, budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool| {
            if insert {
                plan_insert(editor, parsed.query(), budget, cancelled).map_err(
                    |error| match error {
                        InsertError::Query(error) => error,
                        InsertError::Backend(_) => panic!("unexpected backend failure"),
                    },
                )
            } else {
                plan_update(editor, parsed.query(), budget, None, cancelled)
                    .map(|plan| plan.into_backend())
                    .map_err(|error| match error {
                        UpdateError::Query(error) => error,
                        UpdateError::Backend(_) => panic!("unexpected backend failure"),
                    })
            }
        };
        let make_target = || {
            target(
                kind,
                false,
                if insert {
                    vec![]
                } else {
                    vec![Value::Int64(1)]
                },
            )
        };
        let (editor, calls) = make_target();
        let mut measured = budget();
        let start = measured.remaining(Resource::Steps);
        run(editor, &mut measured, &|| false).unwrap();
        assert_eq!(calls.get(), 1);
        let used = start - measured.remaining(Resource::Steps);
        assert!(used > 0);

        let (editor, calls) = make_target();
        let mut exact = ResourceBudget::new(16 * 1024 * 1024, 100_000, used);
        run(editor, &mut exact, &|| false).unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(exact.remaining(Resource::Steps), 0);

        let (editor, calls) = make_target();
        let mut short = ResourceBudget::new(16 * 1024 * 1024, 100_000, used - 1);
        assert!(matches!(
            run(editor, &mut short, &|| false),
            Err(QueryError::Limit(_))
        ));
        assert_eq!(calls.get(), 0);

        let (editor, calls) = make_target();
        assert_eq!(
            run(editor, &mut budget(), &|| true),
            Err(QueryError::Cancelled)
        );
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn smaller_integral_range_boundaries_and_late_overflow_never_publish_a_prefix() {
    for (kind, lower, upper) in [
        (ScalarType::Byte, 0_i64, 255_i64),
        (ScalarType::Int16, -32_768, 32_767),
    ] {
        for integer in [lower, upper] {
            let (editor, calls) = target(kind, false, vec![Value::Int64(integer)]);
            let parsed =
                parse_query("UPDATE Items SET Target=[Input]", ParseLimits::default()).unwrap();
            let plan = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
            assert_eq!(plan.into_backend(), vec![integral(kind, integer)]);
            assert_eq!(calls.get(), 1);

            let (editor, calls) = target(kind, false, vec![]);
            let parsed = parse_query(
                &format!("INSERT INTO Items VALUES ({integer},0)"),
                ParseLimits::default(),
            )
            .unwrap();
            assert_eq!(
                plan_insert(editor, parsed.query(), &mut budget(), &|| false).unwrap(),
                vec![integral(kind, integer), Value::Int64(0)]
            );
            assert_eq!(calls.get(), 1);
        }
        for integer in [lower - 1, upper + 1] {
            let (editor, calls) = target(kind, false, vec![Value::Int64(7), Value::Int64(integer)]);
            let parsed =
                parse_query("UPDATE Items SET Target=[Input]", ParseLimits::default()).unwrap();
            assert!(matches!(
                plan_update(editor, parsed.query(), &mut budget(), None, &|| false),
                Err(UpdateError::Query(QueryError::Evaluation(
                    EvalError::Overflow
                )))
            ));
            assert_eq!(calls.get(), 0);

            let (editor, calls) = target(kind, false, vec![]);
            let parsed = parse_query(
                &format!("INSERT INTO Items VALUES ({integer},0)"),
                ParseLimits::default(),
            )
            .unwrap();
            assert!(matches!(
                plan_insert(editor, parsed.query(), &mut budget(), &|| false),
                Err(InsertError::Query(QueryError::Evaluation(
                    EvalError::Overflow
                )))
            ));
            assert_eq!(calls.get(), 0);
        }
    }
}
