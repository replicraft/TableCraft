//! Original presence is observable at the sole consuming backend finish.
use super::*;
use std::cell::Cell;
use tablecraft_expressions::EvalError;
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};

struct Target<'a> {
    columns: Vec<Column>,
    calls: &'a Cell<usize>,
    companions: bool,
}

enum Finished {
    Complete(Vec<Value>),
    Presence(Vec<InsertValue>),
}

impl<'a> Target<'a> {
    fn new(calls: &'a Cell<usize>, companions: bool) -> Self {
        let mut columns = vec![
            Column::new("ID".to_owned(), ScalarType::Int32, false).unwrap(),
            Column::new("Label".to_owned(), ScalarType::Text, true).unwrap(),
            Column::new("Flag".to_owned(), ScalarType::Boolean, true).unwrap(),
        ];
        if companions {
            columns.remove(0);
        }
        Self {
            columns,
            calls,
            companions,
        }
    }
}

impl InsertableTable for Target<'_> {
    type Error = std::io::Error;
    type Plan = Finished;

    fn name(&self) -> &str {
        "Items"
    }

    fn columns(&self) -> &[Column] {
        &self.columns
    }

    fn schema_policy(&self) -> SchemaPolicy {
        SchemaPolicy {
            max_name_bytes: 255,
            max_columns: 256,
            names: NameComparison::AsciiInsensitive,
        }
    }

    fn requires_field_list(&self) -> bool {
        self.companions
    }

    fn plan_insert(
        self,
        values: Vec<Value>,
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Finished, std::io::Error> {
        self.calls.set(self.calls.get() + 1);
        Ok(Finished::Complete(values))
    }
}

impl OmittingInsertableTable for Target<'_> {
    fn plan_insert_with_omissions(
        self,
        values: Vec<InsertValue>,
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Finished, std::io::Error> {
        self.calls.set(self.calls.get() + 1);
        Ok(Finished::Presence(values))
    }
}

const LIMITS: [u64; 3] = [1024 * 1024, 100, 100_000];

fn budget() -> ResourceBudget {
    ResourceBudget::new(LIMITS[0], LIMITS[1], LIMITS[2])
}

fn query(sql: &str) -> tablecraft_sql::ParsedQuery {
    tablecraft_sql::parse_query(sql, tablecraft_sql::ParseLimits::default()).unwrap()
}

fn presence(sql: &str, companions: bool) -> Vec<InsertValue> {
    let parsed = query(sql);
    let calls = Cell::new(0);
    let result = plan_insert_with_omissions(
        Target::new(&calls, companions),
        parsed.query(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    let Finished::Presence(values) = result else {
        panic!("wrong finish capability")
    };
    values
}

#[test]
fn omitted_required_position_is_intent_and_explicit_null_stays_supplied() {
    let values = presence("INSERT INTO Items(Label) VALUES(NULL)", false);
    assert_eq!(values.len(), 3);
    assert!(matches!(&values[0], InsertValue::Omitted));
    assert!(matches!(&values[1], InsertValue::Supplied(Value::Null)));
    assert!(matches!(&values[2], InsertValue::Omitted));

    let values = presence("INSERT INTO Items(ID,Label) VALUES(7,'null')", false);
    assert!(matches!(&values[0], InsertValue::Supplied(Value::Int32(7))));
    assert!(matches!(&values[1], InsertValue::Supplied(Value::Text(text)) if text == "null"));
    assert!(matches!(&values[2], InsertValue::Omitted));
}

#[test]
fn complete_planner_stays_strict_and_positional_values_require_full_width() {
    let calls = Cell::new(0);
    let partial = query("INSERT INTO Items(ID) VALUES(7)");
    assert!(matches!(
        plan_insert(
            Target::new(&calls, false),
            partial.query(),
            &mut budget(),
            &|| false
        ),
        Err(InsertError::Query(QueryError::Width))
    ));
    assert_eq!(calls.get(), 0);

    let positional = query("INSERT INTO Items VALUES(7,NULL)");
    assert!(matches!(
        plan_insert_with_omissions(
            Target::new(&calls, false),
            positional.query(),
            &mut budget(),
            &|| false
        ),
        Err(InsertError::Query(QueryError::Width))
    ));
    assert_eq!(calls.get(), 0);

    let complete = query("INSERT INTO Items VALUES(7,NULL,TRUE)");
    let result = plan_insert(
        Target::new(&calls, false),
        complete.query(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let Finished::Complete(values) = result else {
        panic!("wrong finish capability")
    };
    assert_eq!(values, [Value::Int32(7), Value::Null, Value::Boolean(true)]);
    assert_eq!(calls.get(), 1);
}

#[test]
fn duplicate_and_unknown_binding_finish_before_any_written_expression() {
    for (sql, duplicate) in [
        ("INSERT INTO Items(ID,id) VALUES(1/0,7)", true),
        ("INSERT INTO Items(Missing) VALUES(1/0)", false),
    ] {
        let parsed = query(sql);
        let calls = Cell::new(0);
        let result = plan_insert_with_omissions(
            Target::new(&calls, false),
            parsed.query(),
            &mut budget(),
            &|| false,
        );
        if duplicate {
            assert!(matches!(
                result,
                Err(InsertError::Query(QueryError::DuplicateInsertField))
            ));
        } else {
            assert!(matches!(
                result,
                Err(InsertError::Query(QueryError::UnknownName))
            ));
        }
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn reordered_subset_uses_written_evaluation_order_and_late_carrier_error_has_no_finish() {
    let values = presence("INSERT INTO Items(Flag,ID) VALUES(TRUE,9)", false);
    assert!(matches!(&values[0], InsertValue::Supplied(Value::Int32(9))));
    assert!(matches!(&values[1], InsertValue::Omitted));
    assert!(matches!(
        &values[2],
        InsertValue::Supplied(Value::Boolean(true))
    ));

    let parsed = query("INSERT INTO Items(Label,ID) VALUES(1/0,2147483648)");
    let calls = Cell::new(0);
    assert!(matches!(
        plan_insert_with_omissions(
            Target::new(&calls, false),
            parsed.query(),
            &mut budget(),
            &|| false
        ),
        Err(InsertError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(calls.get(), 0);

    let parsed = query("INSERT INTO Items(ID,Label) VALUES(7,11)");
    assert!(matches!(
        plan_insert_with_omissions(
            Target::new(&calls, false),
            parsed.query(),
            &mut budget(),
            &|| false
        ),
        Err(InsertError::Query(QueryError::Unsupported))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn generated_companion_schema_is_explicit_and_never_guesses_an_id_omission_slot() {
    let values = presence("INSERT INTO Items(Label) VALUES('x')", true);
    assert_eq!(values.len(), 2);
    assert!(matches!(&values[0], InsertValue::Supplied(Value::Text(text)) if text == "x"));
    assert!(matches!(&values[1], InsertValue::Omitted));

    for sql in [
        "INSERT INTO Items VALUES('x',TRUE)",
        "INSERT INTO Items(ID) VALUES(7)",
    ] {
        let parsed = query(sql);
        let calls = Cell::new(0);
        assert!(
            plan_insert_with_omissions(
                Target::new(&calls, true),
                parsed.query(),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn all_cumulative_pools_and_final_cancellation_fail_before_backend_finish() {
    let parsed = query("INSERT INTO Items(Label,ID) VALUES('payload',7)");
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let calls = Cell::new(0);
    let mut work = budget();
    let polls = Cell::new(0);
    plan_insert_with_omissions(
        Target::new(&calls, false),
        parsed.query(),
        &mut work,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    let needed =
        std::array::from_fn::<_, 3, _>(|index| LIMITS[index] - work.remaining(resources[index]));

    for index in 0..3 {
        let mut exact = LIMITS;
        exact[index] = needed[index];
        let calls = Cell::new(0);
        plan_insert_with_omissions(
            Target::new(&calls, false),
            parsed.query(),
            &mut ResourceBudget::new(exact[0], exact[1], exact[2]),
            &|| false,
        )
        .unwrap();
        assert_eq!(calls.get(), 1);
        exact[index] -= 1;
        let calls = Cell::new(0);
        assert!(
            plan_insert_with_omissions(
                Target::new(&calls, false),
                parsed.query(),
                &mut ResourceBudget::new(exact[0], exact[1], exact[2]),
                &|| false
            )
            .is_err()
        );
        assert_eq!(calls.get(), 0);
    }

    let final_poll = polls.get();
    let polls = Cell::new(0);
    let calls = Cell::new(0);
    assert!(matches!(
        plan_insert_with_omissions(
            Target::new(&calls, false),
            parsed.query(),
            &mut budget(),
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == final_poll
            }
        ),
        Err(InsertError::Query(QueryError::Cancelled))
    ));
    assert_eq!(calls.get(), 0);
}
