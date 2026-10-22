use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn run(sql: &str, format: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::new(vec![]).unwrap();
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: format,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn rounded_operator_inputs_retain_width_and_exact_large_number_promotion() {
    assert_eq!(
        run("SELECT Not 2.5,CCur(2.5) And CByte(3),CDec(3.5) Xor CInt(1),Not CDate(-1.25),CDec(0) Or CLngLng(9007199254740993),VarType(Not 2.5)", None).unwrap(),
        [vec![Value::Int32(-3), Value::Int32(2), Value::Int32(5), Value::Int32(0), Value::Int64(9007199254740993), Value::Int16(3)]]
    );
    for profile in [NumberFormat::EnUs, NumberFormat::DeDe] {
        let text = if profile == NumberFormat::DeDe {
            "2,5"
        } else {
            "2.5"
        };
        assert_eq!(
            run(&format!("SELECT Not '{text}','{text}' And CByte(3),'9007199254740993' Or CLngLng(0),Eval(\"Not '{text}'\")"), Some(profile)).unwrap(),
            [vec![Value::Int32(-3), Value::Int32(2), Value::Int64(9007199254740993), Value::Int32(-3)]]
        );
    }
}

#[test]
fn contextual_predicates_reuse_prepared_plans_in_all_read_consumers() {
    let source = MemorySource::new(vec![]).unwrap();
    let parsed = parse_query("SELECT 99 WHERE '0,25'", ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(EvalError::NumberFormatRequired.into())
    );
    for _ in 0..2 {
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(NumberFormat::DeDe),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![Value::Int64(99)]]
        );
    }
    for (sql, expected) in [
        (
            "SELECT A.ID FROM (SELECT 1 AS ID) AS A INNER JOIN (SELECT 2 AS ID) AS B ON '0,25'",
            vec![vec![Value::Int64(1)]],
        ),
        (
            "SELECT Count(*) FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS A HAVING 'True'",
            vec![vec![Value::Int64(2)]],
        ),
        (
            "SELECT (SELECT Not CDec(2.5)) WHERE 'True'",
            vec![vec![Value::Int32(-3)]],
        ),
        (
            "TRANSFORM Count(*) SELECT K FROM (SELECT 1 AS K,2 AS P) AS A WHERE 'True' GROUP BY K HAVING '0,25' PIVOT P IN (2)",
            vec![vec![Value::Int64(1), Value::Int64(1)]],
        ),
        ("SELECT 99 WHERE 'False'", vec![]),
    ] {
        assert_eq!(
            run(sql, Some(NumberFormat::DeDe)).unwrap(),
            expected,
            "{sql}"
        );
    }
}

#[test]
fn eager_errors_malformed_text_and_overflow_are_not_suppressed() {
    for sql in [
        "SELECT 'bad' And (1/0)",
        "SELECT 0 And (1/0)",
        "SELECT Not (1/0)",
    ] {
        assert_eq!(
            run(sql, Some(NumberFormat::EnUs)),
            Err(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
    for sql in ["SELECT Not '1'", "SELECT '1' And 0", "SELECT 1 WHERE '1'"] {
        assert_eq!(
            run(sql, None),
            Err(QueryError::Evaluation(EvalError::NumberFormatRequired)),
            "{sql}"
        );
    }
    for sql in [
        "SELECT Not 1e40",
        "SELECT CByte(0) And CDec(2147483647.5)",
        "SELECT Not 'bad'",
        "SELECT Not CVErr(0)",
    ] {
        assert!(run(sql, Some(NumberFormat::EnUs)).is_err(), "{sql}");
    }
}
