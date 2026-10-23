use tablecraft_core::values::{Double, Single, Value};
use tablecraft_expressions::EvalError;
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
fn single_selected_overflow_keeps_categories_through_nested_results() {
    for expression in [
        "CSng(3.4028234663852886e38)+CSng(3.4028234663852886e38)",
        "CSng(-3.4028234663852886e38)-CSng(3.4028234663852886e38)",
        "CSng(3.4028234663852886e38)*CInt(2)",
        "CSng(3.4028234663852886e38)/CSng(0.5)",
        "(SELECT Q.N*CInt(2) FROM (SELECT CSng(3.4028234663852886e38) AS N) AS Q)",
    ] {
        let sql = format!("SELECT {expression}");
        assert!(run(&sql, false).is_ok(), "{sql}");
        assert_eq!(
            run(&sql, true),
            Err(QueryError::Evaluation(EvalError::Overflow)),
            "{sql}"
        );
    }
    for expression in [
        "CVar(CSng(3.4028234663852886e38))*CInt(2)",
        "CSng(3.4028234663852886e38)*CVar(CInt(2))",
        "IIf(True,CSng(3.4028234663852886e38),CSng(0))*CInt(2)",
        "(SELECT Q.N*CInt(2) FROM (SELECT CVar(CSng(3.4028234663852886e38)) AS N) AS Q)",
        "CSng(3.4028234663852886e38)*CLng(2)",
    ] {
        assert_eq!(
            run(&format!("SELECT {expression}"), true).unwrap(),
            [vec![Value::Double(Double::from_float(
                f64::from(f32::MAX) * 2.0
            ))]],
            "{expression}"
        );
    }
}

#[test]
fn unknown_categories_only_block_selected_width_operations_and_eager_errors_survive() {
    assert_eq!(
        run("SELECT First(CSng(1))*CInt(2)", true),
        Err(QueryError::ArithmeticDeclaration)
    );
    assert_eq!(
        run("SELECT First(CSng(1))*CLng(2)", true).unwrap(),
        [vec![Value::Double(Double::from_float(2.0))]]
    );
    assert_eq!(
        run("SELECT NULL+(CSng(3.4028234663852886e38)*CInt(2))", true),
        Err(QueryError::Evaluation(EvalError::Overflow))
    );
    assert_eq!(
        run("SELECT NULL+CSng(1)", true).unwrap(),
        [vec![Value::Null]]
    );
    assert_eq!(
        run("SELECT CSng(1)/CSng(0)", true),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    let source = MemorySource::default();
    let parsed = parse_query("PARAMETERS N SINGLE; SELECT N*CInt(2)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for declared in [false, true, false] {
        let result = plan.execute(
            &source,
            &[("N", Value::Single(Single::from_float(f32::MAX)))],
            ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            },
        );
        assert_eq!(result.is_ok(), !declared);
    }
}
