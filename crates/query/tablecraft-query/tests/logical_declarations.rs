use std::cell::Cell;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::math::random::RndState};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
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
fn complements_retain_scalar_and_variant_categories_through_query_scopes() {
    for (input, add, expected) in [
        ("CByte(0)", "CByte(1)", Value::Int16(256)),
        ("CInt(-32768)", "CInt(1)", Value::Int32(32768)),
        (
            "CLng(-2147483648)",
            "CLng(1)",
            Value::Double(Double::from_float(2147483648.0)),
        ),
    ] {
        let expression = format!("Not {input}");
        for sql in [
            format!("SELECT ({expression})+{add}"),
            format!("SELECT Q.N+{add} FROM (SELECT {expression} AS N) AS Q"),
            format!("SELECT (SELECT Q.N+{add}) FROM (SELECT {expression} AS N) AS Q"),
            format!("SELECT Q.N+{add} FROM (SELECT {expression} AS N) AS Q GROUP BY Q.N"),
        ] {
            assert_eq!(run(&sql, true), Err(EvalError::Overflow.into()), "{sql}");
            assert_eq!(
                run(&sql, false).unwrap(),
                vec![vec![expected.clone()]],
                "{sql}"
            );
        }
        for expression in [format!("Not CVar({input})"), format!("CVar(Not {input})")] {
            let sql = format!("SELECT ({expression})+{add}");
            assert_eq!(
                run(&sql, true).unwrap(),
                vec![vec![expected.clone()]],
                "{sql}"
            );
        }
    }
    assert_eq!(
        run("SELECT CInt(-32768)\\(Not False)", true),
        Err(EvalError::Overflow.into())
    );
    assert_eq!(
        run("SELECT CInt(-32768)\\(Not CVar(False))", true).unwrap(),
        vec![vec![Value::Int32(32768)]]
    );
}

#[test]
fn binary_logical_returns_use_both_operand_categories() {
    for (operator, left, right) in [
        ("And", 255, 255),
        ("Or", 255, 0),
        ("Xor", 255, 0),
        ("Eqv", 0, 0),
        ("Imp", 0, 0),
    ] {
        for (left, right, variant) in [
            (format!("CByte({left})"), format!("CByte({right})"), false),
            (
                format!("CVar(CByte({left}))"),
                format!("CByte({right})"),
                true,
            ),
            (
                format!("CByte({left})"),
                format!("CVar(CByte({right}))"),
                true,
            ),
        ] {
            let expression = format!("{left} {operator} {right}");
            for sql in [
                format!("SELECT ({expression})+CByte(1)"),
                format!("SELECT Q.N+CByte(1) FROM (SELECT {expression} AS N) AS Q"),
                format!("SELECT (SELECT Q.N+CByte(1)) FROM (SELECT {expression} AS N) AS Q"),
                format!("SELECT Q.N+CByte(1) FROM (SELECT {expression} AS N) AS Q GROUP BY Q.N"),
            ] {
                if variant {
                    assert_eq!(
                        run(&sql, true).unwrap(),
                        vec![vec![Value::Int16(256)]],
                        "{sql}"
                    );
                } else {
                    assert_eq!(run(&sql, true), Err(EvalError::Overflow.into()), "{sql}");
                }
                assert_eq!(
                    run(&sql, false).unwrap(),
                    vec![vec![Value::Int16(256)]],
                    "{sql}"
                );
            }
        }
    }
}

#[test]
fn nulls_errors_unknown_categories_and_longlong_limits_remain_explicit() {
    for sql in [
        "SELECT CInt(7)\\(Not Count(*))",
        "SELECT CInt(7)\\(Count(*) And CInt(-1))",
    ] {
        assert_eq!(run(sql, true), Err(QueryError::ArithmeticDeclaration));
    }
    let sql = "SELECT CInt(7)\\(Not CVar(Count(*)))";
    assert_eq!(run(sql, true).unwrap(), vec![vec![Value::Int64(-3)]]);
    assert_eq!(
        run("SELECT (Not NULL)+CInt(1)", true).unwrap(),
        vec![vec![Value::Null]]
    );
    for sql in [
        "SELECT (CByte(0) And CVar(1/0))+CByte(1)",
        "SELECT (CVar(1/0) Or CByte(255))+CByte(1)",
        "SELECT (Not CVar(1/0))+CByte(1)",
    ] {
        assert_eq!(
            run(sql, true),
            Err(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT Not CVErr(5)", true),
        Err(EvalError::TypeMismatch.into())
    );
    assert_eq!(
        run("SELECT Not CVar('0')", true),
        Err(EvalError::NumberFormatRequired.into())
    );
    assert_eq!(
        run("SELECT Not CVar(CDbl(2147483648))", true),
        Err(EvalError::Overflow.into())
    );
    let sql = "SELECT (Not CVar(CLngLng(-9223372036854775808)))+CLngLng(1)";
    assert_eq!(run(sql, true), Err(EvalError::Overflow.into()));
}

#[test]
fn logical_operands_are_evaluated_once_with_shared_cancellation() {
    let source = MemorySource::default();
    let random = RndState::new(7).unwrap();
    let reference = RndState::new(7).unwrap();
    reference.apply(None).unwrap();
    reference.apply(None).unwrap();
    let parsed = parse_query(
        "SELECT (Not CInt(Rnd()*100)) Xor CInt(Rnd()*100)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let calls = Cell::new(0_u64);
    let cancelled = || {
        calls.set(calls.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            random: Some(&random),
            cancelled: Some(&cancelled),
            declared_integral: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(calls.get() > 0);
    assert_eq!(random.apply(None).unwrap(), reference.apply(None).unwrap());
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| true),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Cancelled)
    );
}
