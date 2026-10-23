use tablecraft_core::values::{Double, Single, Value};
use tablecraft_expressions::{EvalError, functions::math::random::RndState};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
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
fn variant_math_returns_do_not_inherit_scalar_argument_categories() {
    for function in ["Abs", "Int", "Fix", "Round"] {
        for (input, operation, expected) in [
            ("CByte(255)", "+CByte(1)", Value::Int16(256)),
            ("CInt(32767)", "+CInt(1)", Value::Int32(32768)),
            (
                "CSng(3.4028234663852886e38)",
                "*CInt(2)",
                Value::Double(Double::from_float(f64::from(f32::MAX) * 2.0)),
            ),
        ] {
            for sql in [
                format!("SELECT {function}({input}){operation}"),
                format!("SELECT Q.N{operation} FROM (SELECT {function}({input}) AS N) AS Q"),
                format!(
                    "SELECT (SELECT Q.N{operation}) FROM (SELECT {function}({input}) AS N) AS Q"
                ),
                format!(
                    "SELECT {function}({input}){operation} FROM (SELECT 1 AS N) AS Q GROUP BY {function}({input})"
                ),
            ] {
                for declared in [false, true] {
                    assert_eq!(
                        run(&sql, declared).unwrap(),
                        [vec![expected.clone()]],
                        "{sql}"
                    );
                }
            }
        }
        assert_eq!(
            run(
                &format!("SELECT CInt({function}(CInt(32767)))+CInt(1)"),
                true
            ),
            Err(EvalError::Overflow.into())
        );
    }
}

#[test]
fn scalar_transcendental_returns_do_not_inherit_variant_argument_categories() {
    for expression in [
        "Sqr(CVar(1))",
        "Exp(CVar(0))",
        "Log(CVar(1))+CInt(1)",
        "Sin(CVar(0))+CInt(1)",
        "Cos(CVar(0))",
        "Tan(CVar(0))+CInt(1)",
        "Atn(CVar(0))+CInt(1)",
    ] {
        let sql = format!("SELECT CInt(-32768)\\(-({expression}))");
        assert_eq!(run(&sql, true), Err(EvalError::Overflow.into()), "{sql}");
        assert_eq!(run(&sql, false).unwrap(), [vec![Value::Int32(32768)]]);
        let sql = format!("SELECT CInt(-32768)\\CVar(-({expression}))");
        assert_eq!(run(&sql, true).unwrap(), [vec![Value::Int32(32768)]]);
    }
}

#[test]
fn rnd_scalar_return_checks_single_overflow_without_extra_draws() {
    let source = MemorySource::default();
    let state = RndState::new(1 << 23).unwrap();
    for (sql, succeeds) in [
        ("SELECT CSng(3.4028234663852886e38)/Rnd(0)", false),
        ("SELECT CSng(3.4028234663852886e38)/CVar(Rnd(0))", true),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source).unwrap().execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                random: Some(&state),
                ..Default::default()
            },
        );
        if succeeds {
            assert_eq!(
                result.unwrap().rows(),
                &[vec![Value::Double(Double::from_float(
                    f64::from(f32::MAX) * 2.0
                ))]]
            );
        } else {
            assert_eq!(result.err(), Some(EvalError::Overflow.into()));
        }
        assert_eq!(
            state.apply(Some(&Value::Int32(0))).unwrap(),
            Value::Single(Single::from_float(0.5))
        );
    }
    let reference = RndState::new(1 << 23).unwrap();
    let expected = reference.apply(None).unwrap();
    let parsed = parse_query("SELECT Rnd()*CInt(1)", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    random: Some(&state),
                    declared_integral: true,
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
        &[vec![expected]]
    );
    assert_eq!(state.apply(None).unwrap(), reference.apply(None).unwrap());
}

#[test]
fn len_variant_returns_keep_long_carriers_and_explicit_cast_overflow() {
    for (expression, expected) in [("Len('a')", 2147483648.0), ("LenB('a')", 2147483649.0)] {
        assert_eq!(
            run(
                &format!("SELECT Q.N+CLng(2147483647) FROM (SELECT {expression} AS N) AS Q"),
                true
            )
            .unwrap(),
            [vec![Value::Double(Double::from_float(expected))]]
        );
        assert_eq!(
            run(&format!("SELECT CLng({expression})+CLng(2147483647)"), true),
            Err(EvalError::Overflow.into())
        );
    }
    assert_eq!(
        run("SELECT Abs(NULL)+CInt(1),Len(NULL)+CInt(1)", true).unwrap(),
        [vec![Value::Null, Value::Null]]
    );
    assert_eq!(
        run("SELECT Abs(1/0)+CInt(1)", true),
        Err(EvalError::DivisionByZero.into())
    );
}
