use tablecraft_core::values::Value;
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
fn list_and_range_categories_include_every_operand_through_query_scopes() {
    for (expression, variant) in [
        ("CInt(1) BETWEEN CInt(0) AND CInt(2)", false),
        ("CVar(CInt(1)) BETWEEN CInt(0) AND CInt(2)", true),
        ("CInt(1) BETWEEN CVar(CInt(0)) AND CInt(2)", true),
        ("CInt(1) BETWEEN CInt(0) AND CVar(CInt(2))", true),
        ("CInt(1) BETWEEN CVar(CInt(2)) AND CInt(0)", true),
        ("CInt(3) NOT BETWEEN CInt(0) AND CInt(2)", false),
        ("CInt(3) NOT BETWEEN CInt(0) AND CVar(CInt(2))", true),
        ("CInt(1) IN (CInt(1),CInt(2))", false),
        ("CVar(CInt(1)) IN (CInt(1),CInt(2))", true),
        ("CInt(1) IN (CVar(CInt(1)),CInt(2))", true),
        ("CInt(1) IN (CInt(1),CVar(CInt(2)))", true),
        ("CInt(3) NOT IN (CInt(1),CInt(2))", false),
        ("CInt(3) NOT IN (CInt(1),CVar(CInt(2)))", true),
    ] {
        for sql in [
            format!("SELECT CInt(-32768)\\({expression})"),
            format!("SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q"),
            format!("SELECT (SELECT CInt(-32768)\\Q.N) FROM (SELECT {expression} AS N) AS Q"),
            format!("SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q GROUP BY Q.N"),
        ] {
            if variant {
                assert_eq!(
                    run(&sql, true).unwrap(),
                    vec![vec![Value::Int32(32768)]],
                    "{sql}"
                );
            } else {
                assert_eq!(run(&sql, true), Err(EvalError::Overflow.into()), "{sql}");
            }
            assert_eq!(
                run(&sql, false).unwrap(),
                vec![vec![Value::Int32(32768)]],
                "{sql}"
            );
        }
        let sql = format!("SELECT CInt(-32768)\\CBool({expression})");
        assert_eq!(run(&sql, true), Err(EvalError::Overflow.into()), "{sql}");
    }
}

#[test]
fn unknowns_nulls_error_data_and_late_failures_keep_their_boundaries() {
    for expression in [
        "Count(*) BETWEEN CInt(0) AND CInt(2)",
        "CInt(1) IN (CInt(1),First(CInt(2)))",
    ] {
        let sql = format!("SELECT CInt(-32768)\\({expression})");
        assert_eq!(
            run(&sql, true),
            Err(QueryError::ArithmeticDeclaration),
            "{sql}"
        );
    }
    for expression in [
        "Count(*) BETWEEN CInt(0) AND CVar(CInt(2))",
        "CInt(1) IN (CInt(1),CVar(First(CInt(2))))",
        "CVErr(1) IN (CVErr(0),CVErr(1))",
        "CVErr(1) BETWEEN CVErr(0) AND CVErr(2)",
    ] {
        let sql = format!("SELECT CInt(-32768)\\({expression})");
        assert_eq!(
            run(&sql, true).unwrap(),
            vec![vec![Value::Int32(32768)]],
            "{sql}"
        );
    }
    for expression in [
        "NULL IN (CInt(1),CVar(CInt(2)))",
        "CInt(1) NOT IN (CInt(2),NULL)",
        "CInt(1) BETWEEN NULL AND CInt(2)",
    ] {
        let sql = format!("SELECT CInt(-32768)\\({expression})");
        assert_eq!(run(&sql, true).unwrap(), vec![vec![Value::Null]], "{sql}");
    }
    for sql in [
        "SELECT CInt(1) IN (CInt(1),CVar(1/0))",
        "SELECT CVar(1) BETWEEN CInt(0) AND 1/0",
    ] {
        assert_eq!(
            run(sql, true),
            Err(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT CInt(1) IN (CInt(1),CVErr(1))", true),
        Err(EvalError::TypeMismatch.into())
    );
}

#[test]
fn existence_is_scalar_without_inspecting_subquery_value_declarations() {
    for expression in [
        "EXISTS(SELECT CVar(NULL))",
        "EXISTS(SELECT CVErr(0))",
        "NOT EXISTS(SELECT CVar(NULL) WHERE False)",
    ] {
        for sql in [
            format!("SELECT CInt(-32768)\\({expression})"),
            format!("SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q"),
        ] {
            assert_eq!(run(&sql, true), Err(EvalError::Overflow.into()), "{sql}");
        }
        let sql = format!("SELECT CInt(-32768)\\CVar({expression})");
        assert_eq!(
            run(&sql, true).unwrap(),
            vec![vec![Value::Int32(32768)]],
            "{sql}"
        );
    }
    for expression in [
        "CVar(1) IN (SELECT CVar(1))",
        "CVar(1) NOT IN (SELECT CVar(1) WHERE False)",
        "CVar(1)=ALL(SELECT CVar(1) WHERE False)",
        "CVar(1)=ANY(SELECT CVar(1))",
    ] {
        let sql = format!("SELECT CInt(-32768)\\({expression})");
        assert_eq!(
            run(&sql, true).unwrap(),
            vec![vec![Value::Int32(32768)]],
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT EXISTS(SELECT 1/0)", true),
        Err(EvalError::DivisionByZero.into())
    );
    for expression in [
        "CInt(1) IN (SELECT Count(*))",
        "CInt(1) NOT IN (SELECT Count(*) FROM (SELECT 1 AS N WHERE False) AS Q GROUP BY N)",
        "CInt(1)=ALL(SELECT Count(*) FROM (SELECT 1 AS N WHERE False) AS Q GROUP BY N)",
    ] {
        let sql = format!("SELECT CInt(-32768)\\({expression})");
        assert_eq!(
            run(&sql, true),
            Err(QueryError::ArithmeticDeclaration),
            "{sql}"
        );
    }
}

#[test]
fn range_and_list_operands_are_not_reevaluated_for_declarations() {
    let source = MemorySource::default();
    for sql in [
        "SELECT CVar(Rnd()) BETWEEN Rnd() AND Rnd()",
        "SELECT CVar(Rnd()) IN (Rnd(),Rnd())",
    ] {
        let random = RndState::new(7).unwrap();
        let reference = RndState::new(7).unwrap();
        for _ in 0..3 {
            reference.apply(None).unwrap();
        }
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                random: Some(&random),
                ..Default::default()
            },
        )
        .unwrap();
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
}
