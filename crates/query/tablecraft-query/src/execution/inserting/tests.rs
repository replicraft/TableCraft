use super::*;
use std::cell::Cell;
use tablecraft_expressions::EvalError;
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};

struct Target<'a> {
    columns: Vec<Column>,
    finish: &'a Cell<usize>,
    companions: bool,
}

impl<'a> Target<'a> {
    fn ordinary(finish: &'a Cell<usize>) -> Self {
        Self {
            columns: vec![
                Column::new("ID".to_owned(), ScalarType::Int32, false).unwrap(),
                Column::new("Name".to_owned(), ScalarType::Text, true).unwrap(),
            ],
            finish,
            companions: false,
        }
    }
}

impl InsertableTable for Target<'_> {
    type Error = std::io::Error;
    type Plan = Vec<Value>;

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
    ) -> Result<Vec<Value>, std::io::Error> {
        self.finish.set(self.finish.get() + 1);
        Ok(values)
    }
}

const LIMITS: [u64; 3] = [1024 * 1024, 100, 100_000];

fn budget() -> ResourceBudget {
    ResourceBudget::new(LIMITS[0], LIMITS[1], LIMITS[2])
}

fn query(sql: &str) -> tablecraft_sql::ParsedQuery {
    tablecraft_sql::parse_query(sql, tablecraft_sql::ParseLimits::default()).unwrap()
}

#[test]
fn complete_fields_reorder_to_schema_and_only_one_backend_finish_occurs() {
    for (sql, wanted) in [
        (
            "INSERT INTO Items (Name,ID) VALUES ('literal null',2+2)",
            vec![Value::Int32(4), Value::Text("literal null".into())],
        ),
        (
            "INSERT INTO Items VALUES (5,NULL)",
            vec![Value::Int32(5), Value::Null],
        ),
        (
            "INSERT INTO Items (id,name) VALUES (CLng(6),'')",
            vec![Value::Int32(6), Value::Text(String::new())],
        ),
    ] {
        let parsed = query(sql);
        let calls = Cell::new(0);
        let result = plan_insert(
            Target::ordinary(&calls),
            parsed.query(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(result, wanted);
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn missing_duplicate_unknown_and_wrong_carriers_refuse_before_finish() {
    for sql in [
        "INSERT INTO Items(ID) VALUES (4)",
        "INSERT INTO Items(ID,id) VALUES (4,5)",
        "INSERT INTO Items(ID,Missing) VALUES (4,'x')",
        "INSERT INTO Items(ID,Name) VALUES (NULL,'x')",
        "INSERT INTO Items(ID,Name) VALUES (4,7)",
        "INSERT INTO Other(ID,Name) VALUES (4,'x')",
        "INSERT INTO Items(ID,Name) VALUES (2147483648,'x')",
    ] {
        let parsed = query(sql);
        let calls = Cell::new(0);
        assert!(
            plan_insert(
                Target::ordinary(&calls),
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
fn positional_values_never_implicitly_omit_a_generated_field() {
    for (sql, accepted) in [
        ("INSERT INTO Items(Name) VALUES ('x')", true),
        ("INSERT INTO Items VALUES ('x')", false),
        ("INSERT INTO Items(ID) VALUES (1)", false),
    ] {
        let parsed = query(sql);
        let calls = Cell::new(0);
        let target = Target {
            columns: vec![Column::new("Name".into(), ScalarType::Text, true).unwrap()],
            finish: &calls,
            companions: true,
        };
        assert_eq!(
            plan_insert(target, parsed.query(), &mut budget(), &|| false).is_ok(),
            accepted
        );
        assert_eq!(calls.get(), usize::from(accepted));
    }
    let calls = Cell::new(0);
    let parsed = query("INSERT INTO Items VALUES (1)");
    let target = Target {
        columns: Vec::new(),
        finish: &calls,
        companions: false,
    };
    assert!(matches!(
        plan_insert(target, parsed.query(), &mut budget(), &|| false),
        Err(InsertError::Query(QueryError::Width))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn select_parameters_fields_identity_and_calls_are_closed_before_finish() {
    for sql in [
        "INSERT INTO Items SELECT ID,Name FROM Original",
        "INSERT INTO Items(ID,Name) VALUES (OtherID,'x')",
        "INSERT INTO Items(ID,Name) VALUES (@@IDENTITY,'x')",
        "INSERT INTO Items(ID,Name) VALUES (UserFunction(),'x')",
        "INSERT INTO Items(ID,Name) VALUES (Eval('1'),'x')",
        "INSERT INTO Items(ID,Name) VALUES (Date(),'x')",
    ] {
        let parsed = query(sql);
        let calls = Cell::new(0);
        assert!(local_insert_table(parsed.query(), false, &mut budget(), &|| false).is_err());
        assert!(
            plan_insert(
                Target::ordinary(&calls),
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
fn late_scalar_errors_return_no_backend_prefix() {
    let parsed = query("INSERT INTO Items(ID,Name) VALUES (4,1/0)");
    let calls = Cell::new(0);
    assert!(matches!(
        plan_insert(
            Target::ordinary(&calls),
            parsed.query(),
            &mut budget(),
            &|| false
        ),
        Err(InsertError::Query(QueryError::Evaluation(_)))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn all_pools_and_terminal_cancellation_precede_backend_finish() {
    let parsed = query("INSERT INTO Items(Name,ID) VALUES ('complete',4)");
    let calls = Cell::new(0);
    let polls = Cell::new(0);
    let mut healthy = budget();
    plan_insert(
        Target::ordinary(&calls),
        parsed.query(),
        &mut healthy,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    let required = [
        LIMITS[0] - healthy.remaining(Resource::Bytes),
        LIMITS[1] - healthy.remaining(Resource::Rows),
        LIMITS[2] - healthy.remaining(Resource::Steps),
    ];
    let mut exact = ResourceBudget::new(required[0], required[1], required[2]);
    calls.set(0);
    plan_insert(
        Target::ordinary(&calls),
        parsed.query(),
        &mut exact,
        &|| false,
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        assert_eq!(exact.remaining(resource), 0);
    }

    for (position, resource) in [Resource::Bytes, Resource::Rows, Resource::Steps]
        .into_iter()
        .enumerate()
    {
        let used = LIMITS[position] - healthy.remaining(resource);
        assert!(used > 0);
        let mut limits = LIMITS;
        limits[position] = used - 1;
        let mut limited = ResourceBudget::new(limits[0], limits[1], limits[2]);
        calls.set(0);
        assert!(
            matches!(plan_insert(Target::ordinary(&calls), parsed.query(), &mut limited, &|| false), Err(InsertError::Query(QueryError::Limit(error))) if error.resource == resource)
        );
        assert_eq!(calls.get(), 0);
    }
    let terminal = polls.get();
    polls.set(0);
    calls.set(0);
    assert!(matches!(
        plan_insert(
            Target::ordinary(&calls),
            parsed.query(),
            &mut budget(),
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == terminal
            }
        ),
        Err(InsertError::Query(QueryError::Cancelled))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn field_identity_binding_precedes_all_values_and_preserves_written_error_order() {
    for (sql, expected) in [
        (
            "INSERT INTO Items(ID,id) VALUES (1/0,1/0)",
            QueryError::DuplicateInsertField,
        ),
        (
            "INSERT INTO Items(ID,Missing) VALUES (1/0,1/0)",
            QueryError::UnknownName,
        ),
        (
            "INSERT INTO Items(Name,ID) VALUES (1/0,2147483648)",
            QueryError::Evaluation(EvalError::DivisionByZero),
        ),
        (
            "INSERT INTO Items(ID,Name) VALUES (2147483648,1/0)",
            QueryError::Evaluation(EvalError::Overflow),
        ),
    ] {
        let calls = Cell::new(0);
        let parsed = query(sql);
        let result = plan_insert(
            Target::ordinary(&calls),
            parsed.query(),
            &mut budget(),
            &|| false,
        );
        assert!(
            matches!(result, Err(InsertError::Query(actual)) if actual == expected),
            "{sql}"
        );
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn declared_non_int32_carriers_require_exact_existing_constructors() {
    for (kind, constructor) in [
        (ScalarType::Boolean, "CBool(7)"),
        (ScalarType::Byte, "CByte(7)"),
        (ScalarType::Int16, "CInt(7)"),
        (ScalarType::Single, "CSng(7)"),
        (ScalarType::Double, "CDbl(7)"),
        (ScalarType::Currency, "CCur(7)"),
        (ScalarType::Decimal, "CDec(7)"),
    ] {
        let calls = Cell::new(0);
        let target = Target {
            columns: vec![Column::new("Amount".into(), kind, false).unwrap()],
            finish: &calls,
            companions: false,
        };
        let parsed = query(&format!("INSERT INTO Items(Amount) VALUES ({constructor})"));
        let values = plan_insert(target, parsed.query(), &mut budget(), &|| false).unwrap();
        assert_eq!(ScalarType::of(&values[0]), Some(kind));
        assert_eq!(calls.get(), 1);

        let calls = Cell::new(0);
        let target = Target {
            columns: vec![Column::new("Amount".into(), kind, false).unwrap()],
            finish: &calls,
            companions: false,
        };
        let parsed = query("INSERT INTO Items(Amount) VALUES (7)");
        assert!(matches!(
            plan_insert(target, parsed.query(), &mut budget(), &|| false),
            Err(InsertError::Query(QueryError::Unsupported))
        ));
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn duplicate_or_overwide_metadata_is_bounded_before_expression_evaluation() {
    let parsed = query("INSERT INTO Items VALUES (1/0,1/0)");
    let calls = Cell::new(0);
    let target = Target {
        columns: vec![
            Column::new("Score".into(), ScalarType::Int32, false).unwrap(),
            Column::new("score".into(), ScalarType::Int32, false).unwrap(),
        ],
        finish: &calls,
        companions: false,
    };
    assert!(matches!(
        plan_insert(target, parsed.query(), &mut budget(), &|| false),
        Err(InsertError::Query(QueryError::AmbiguousName))
    ));
    assert_eq!(calls.get(), 0);

    let parsed = query("INSERT INTO Items VALUES (1/0)");
    let calls = Cell::new(0);
    let target = Target {
        columns: vec![Column::new("x".repeat(256), ScalarType::Int32, false).unwrap()],
        finish: &calls,
        companions: false,
    };
    assert!(matches!(
        plan_insert(target, parsed.query(), &mut budget(), &|| false),
        Err(InsertError::Query(QueryError::Schema(_)))
    ));
    assert_eq!(calls.get(), 0);
}
