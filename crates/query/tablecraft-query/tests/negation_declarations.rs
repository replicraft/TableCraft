use std::cell::Cell;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let query = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn unchanged_negation_carriers_do_not_require_an_unknown_declaration() {
    for value in [
        "CInt(-32767)",
        "CInt(-1)",
        "CInt(0)",
        "CInt(1)",
        "CInt(32767)",
        "CLng(-2147483647)",
        "CLng(-32768)",
        "CLng(-1)",
        "CLng(0)",
        "CLng(1)",
        "CLng(2147483647)",
        "CDate(-657434.75)",
        "CDate(-1.25)",
        "CDate(-0.25)",
        "CDate(0)",
        "CDate(0.25)",
        "CDate(657434.75)",
    ] {
        let expected = run(&format!("SELECT -{value}"), true).unwrap();
        for aggregate in ["First", "Last", "Min", "Max"] {
            let sql = format!("SELECT -{aggregate}({value})");
            assert_eq!(run(&sql, true).unwrap(), expected, "{sql}");
        }

        for sql in [
            format!(
                "SELECT -Q.N FROM (SELECT {value} AS N UNION ALL SELECT {value} WHERE False) AS Q"
            ),
            format!(
                "SELECT (SELECT -Q.N) FROM (SELECT {value} AS N UNION ALL SELECT {value} WHERE False) AS Q"
            ),
            format!("SELECT -Q.N FROM (SELECT First({value}) AS N) AS Q"),
        ] {
            assert_eq!(run(&sql, true).unwrap(), expected, "{sql}");
        }
    }
}

#[test]
fn differing_scalar_variant_outcomes_still_require_explicit_provenance() {
    for value in ["CInt(-32768)", "CLng(-2147483648)", "CDate(2958465)"] {
        for sql in [
            format!("SELECT -First({value})"),
            format!(
                "SELECT -Q.N FROM (SELECT {value} AS N UNION ALL SELECT {value} WHERE False) AS Q"
            ),
        ] {
            assert_eq!(
                run(&sql, true),
                Err(QueryError::ArithmeticDeclaration),
                "{sql}"
            );
            assert!(run(&sql, false).is_ok(), "{sql}");
        }
        assert_eq!(
            run(&format!("SELECT -{value}"), true),
            Err(QueryError::Evaluation(EvalError::Overflow))
        );
        assert_eq!(
            run(&format!("SELECT -CVar(First({value}))"), true).unwrap(),
            run(&format!("SELECT -{value}"), false).unwrap()
        );
    }

    assert_eq!(
        run("SELECT -CVar(First(CDate(2958465)))", true).unwrap(),
        [vec![Value::Double(Double::from_float(-2958465.0))]]
    );
}

#[test]
fn agreement_does_not_assign_a_declaration_or_hide_operand_failures() {
    for sql in [
        "SELECT -First(CInt(2))+CInt(1)",
        "SELECT Q.N+CInt(1) FROM (SELECT -First(CInt(2)) AS N) AS Q",
        "SELECT -First(CDate(2))+CDate(1)",
    ] {
        assert_eq!(
            run(sql, true),
            Err(QueryError::ArithmeticDeclaration),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT CVar(-First(CInt(2)))+CInt(1)", true).unwrap(),
        [vec![Value::Int16(-1)]]
    );
    assert_eq!(
        run("SELECT -First(CInt(1/0))", true),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT -First(NULL)", true).unwrap(),
        [vec![Value::Null]]
    );
}

#[test]
fn prepared_unknown_operands_rebind_after_failure_and_keep_cancellation_checks() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS P SHORT; SELECT -Q.N FROM (SELECT P AS N UNION ALL SELECT P WHERE False) AS Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();

    for value in [2, i16::MIN, -2, i16::MAX] {
        let result = plan.execute(
            &source,
            &[("P", Value::Int16(value))],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        );
        if value == i16::MIN {
            assert_eq!(result.err(), Some(QueryError::ArithmeticDeclaration));
        } else {
            assert_eq!(result.unwrap().rows(), [vec![Value::Int16(-value)]]);
        }
    }

    let checks = Cell::new(0);
    plan.execute(
        &source,
        &[("P", Value::Int16(2))],
        ExecutionOptions {
            declared_integral: true,
            cancelled: Some(&|| {
                checks.set(checks.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();
    let total = checks.get();
    assert!(total > 0);

    for stop in 1..=total {
        checks.set(0);
        let result = plan.execute(
            &source,
            &[("P", Value::Int16(2))],
            ExecutionOptions {
                declared_integral: true,
                cancelled: Some(&|| {
                    checks.set(checks.get() + 1);
                    checks.get() >= stop
                }),
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
    }

    assert_eq!(
        plan.execute(
            &source,
            &[("P", Value::Int16(2))],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![Value::Int16(-2)]]
    );
}
