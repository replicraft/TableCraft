//! Explicit preparation-time literal carriers, independent of native qualification.
use tablecraft_core::values::{Double, ExpressionValue, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, execute_with_schema_discovery, prepare,
    prepare_with_options, source::MemorySource,
};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    let result = execute_with_schema_discovery(
        parsed.query(),
        &source,
        &[],
        ExecutionOptions {
            declared_integral: true,
            number_format: Some(NumberFormat::EnUs),
            ..Default::default()
        },
    )?;
    result
        .rows()
        .iter()
        .map(|row| {
            row.iter()
                .map(|value| match value {
                    ExpressionValue::Value(value) => Ok(value.clone()),
                    _ => Err(QueryError::Unsupported),
                })
                .collect()
        })
        .collect()
}

#[test]
fn decimal_tokens_select_integer_long_then_double_from_unsigned_magnitude() {
    for (text, expected) in [
        ("0", Value::Int16(0)),
        ("32767", Value::Int16(32767)),
        ("32768", Value::Int32(32768)),
        ("-32767", Value::Int16(-32767)),
        ("-32768", Value::Int32(-32768)),
        ("2147483647", Value::Int32(2147483647)),
        ("-2147483647", Value::Int32(-2147483647)),
        (
            "2147483648",
            Value::Double(Double::from_float(2147483648.0)),
        ),
        (
            "-2147483648",
            Value::Double(Double::from_float(-2147483648.0)),
        ),
        (
            "9007199254740991",
            Value::Double(Double::from_float(9007199254740991.0)),
        ),
        (
            "9007199254740992",
            Value::Double(Double::from_float(9007199254740992.0)),
        ),
        (
            "9007199254740993",
            Value::Double(Double::from_float(9007199254740992.0)),
        ),
        (
            "9223372036854775807",
            Value::Double(Double::from_float(9223372036854775808.0)),
        ),
        (
            "9223372036854775808",
            Value::Double(Double::from_float(9223372036854775808.0)),
        ),
        (
            "-9223372036854775808",
            Value::Double(Double::from_float(-9223372036854775808.0)),
        ),
        (
            "-9223372036854775809",
            Value::Double(Double::from_float(-9223372036854775808.0)),
        ),
        (
            "79228162514264337593543950335",
            Value::Double(Double::from_float(7.922816251426434e28)),
        ),
        ("1.0", Value::Double(Double::from_float(1.0))),
        ("1e0", Value::Double(Double::from_float(1.0))),
        (
            "9.007199254740993e15",
            Value::Double(Double::from_float(9007199254740992.0)),
        ),
    ] {
        assert_eq!(
            run(&format!("SELECT {text}")).unwrap(),
            [vec![expected]],
            "{text}"
        );
    }
}

#[test]
fn scalar_overflow_variant_widening_and_written_eager_errors_use_literal_carriers() {
    for sql in ["SELECT 32767+1", "SELECT 2147483647+1", "SELECT 300*300"] {
        assert_eq!(
            run(sql),
            Err(QueryError::Evaluation(EvalError::Overflow)),
            "{sql}"
        );
    }
    for sql in [
        "SELECT CVar(32767)+1",
        "SELECT IIf(True,32767,0)+1",
        "SELECT Nz(32767,0)+1",
        "SELECT Q.N+1 FROM (SELECT CVar(32767) AS N) AS Q",
        "SELECT (SELECT Q.N+1) FROM (SELECT CVar(32767) AS N) AS Q",
    ] {
        assert_eq!(run(sql).unwrap(), [vec![Value::Int32(32768)]], "{sql}");
    }
    assert_eq!(run("SELECT Null+32767"), Ok(vec![vec![Value::Null]]));
    assert_eq!(
        run("SELECT IIf(True,1,1/0)"),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT Null+(1/0)"),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT CVar(2147483647)+1").unwrap(),
        [vec![Value::Double(Double::from_float(2147483648.0))]]
    );
}

#[test]
fn explicit_casts_and_decimal_preserve_their_contracts_after_token_materialization() {
    assert_eq!(
        run("SELECT CLngLng('9223372036854775807')").unwrap(),
        [vec![Value::Int64(i64::MAX)]]
    );
    assert_eq!(
        run("SELECT CLngLng(9223372036854775807)"),
        Err(QueryError::Evaluation(EvalError::Overflow))
    );
    assert_eq!(
        run("SELECT CLngLng(-9223372036854775808)").unwrap(),
        [vec![Value::Int64(i64::MIN)]]
    );
    assert_eq!(run("SELECT VarType(CDec('79228162514264337593543950335')),VarType(CLng(1)),VarType(CInt(1))").unwrap(), [vec![Value::Int16(14), Value::Int16(3), Value::Int16(2)]]);
    assert_eq!(
        run("PARAMETERS P LONG; SELECT VarType(P),P+1").unwrap_err(),
        QueryError::Parameter
    );
}

#[test]
fn preparation_captures_types_and_default_preparation_remains_unchanged() {
    let source = MemorySource::default();
    let parsed = parse_query("SELECT 1,32768,2147483648", Default::default()).unwrap();
    let default_plan = prepare(parsed.query(), &source).unwrap();
    let typed_plan = prepare_with_options(
        parsed.query(),
        &source,
        PreparationOptions {
            declared_arithmetic: true,
            ..Default::default()
        },
    )
    .unwrap();
    for declared_integral in [false, true] {
        let options = ExecutionOptions {
            declared_integral,
            ..Default::default()
        };
        assert_eq!(
            default_plan.execute(&source, &[], options).unwrap().rows(),
            [vec![
                Value::Int64(1),
                Value::Int64(32768),
                Value::Int64(2147483648)
            ]]
        );
        assert_eq!(
            typed_plan.execute(&source, &[], options).unwrap().rows(),
            [vec![
                Value::Int16(1),
                Value::Int32(32768),
                Value::Double(Double::from_float(2147483648.0))
            ]]
        );
    }
    assert_ne!(
        default_plan.output_types().unwrap(),
        typed_plan.output_types().unwrap()
    );
}

#[test]
fn crosstab_metadata_and_execution_share_selected_width_overflow() {
    for sql in [
        "TRANSFORM Sum(V) SELECT K FROM (SELECT 1 AS K,1 AS V) AS T GROUP BY K PIVOT 1 IN (32767+1)",
        "TRANSFORM Sum(V) SELECT K FROM (SELECT 1 AS K,1 AS V) AS T GROUP BY K PIVOT 1 IN (-CInt(-32768))",
    ] {
        assert_eq!(
            run(sql),
            Err(QueryError::Evaluation(EvalError::Overflow)),
            "{sql}"
        );
    }

    let sql = "TRANSFORM Sum(V) SELECT K FROM (SELECT 1 AS K,1 AS V) AS T GROUP BY K PIVOT CVar(32767)+1 IN (CVar(32767)+1)";
    assert_eq!(run(sql).unwrap(), [vec![Value::Int16(1), Value::Int64(1)]]);
}

#[test]
fn typed_parameters_are_not_reinferred_and_literal_limits_remain_bounded() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P LONGLONG; SELECT VarType(P),P",
        Default::default(),
    )
    .unwrap();
    let plan = prepare_with_options(
        parsed.query(),
        &source,
        PreparationOptions {
            declared_arithmetic: true,
            ..Default::default()
        },
    )
    .unwrap();
    let result = plan
        .execute(
            &source,
            &[("P", Value::Int64(i64::MAX))],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![Value::Int16(20), Value::Int64(i64::MAX)]]
    );

    assert!(run(&format!("SELECT {}", "9".repeat(128))).is_ok());
    assert_eq!(
        run(&format!("SELECT {}", "9".repeat(129))),
        Err(QueryError::Evaluation(EvalError::UnsupportedLiteral))
    );

    let parsed = parse_query("SELECT 32768", Default::default()).unwrap();
    assert!(matches!(
        execute_with_schema_discovery(
            parsed.query(),
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                cancelled: Some(&|| true),
                ..Default::default()
            }
        ),
        Err(QueryError::Cancelled)
    ));
}

#[test]
fn captured_literals_do_not_increase_structural_depth_of_existing_queries() {
    let sql =
        "SELECT CInt(-32768)\\(CInt(1) IN (SELECT Q.N FROM (SELECT CVar(CInt(1)) AS N) AS Q))";
    assert_eq!(run(sql).unwrap(), [vec![Value::Int32(32768)]]);
}
