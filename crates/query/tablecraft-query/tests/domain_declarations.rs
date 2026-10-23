use tablecraft_core::values::{Double, ExpressionValue, Single, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("ID".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Octet".into(), ScalarType::Byte, true).unwrap(),
                Column::new("Small".into(), ScalarType::Int16, true).unwrap(),
                Column::new("Real".into(), ScalarType::Single, true).unwrap(),
            ],
            (1..=2)
                .map(|id| {
                    vec![
                        Value::Int32(id),
                        Value::Byte(255),
                        Value::Int16(32767),
                        Value::Single(Single::from_float(f32::MAX)),
                    ]
                })
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn run(sql: &str, source: &MemorySource, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(
            source,
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
fn domain_selection_returns_variant_independently_of_source_carriers() {
    let source = source();
    for function in ["DLookup", "DFirst", "DLast", "DMin", "DMax"] {
        for (argument, operation, expected) in [
            ("Octet", "+CByte(1)", Value::Int16(256)),
            ("Small", "+CInt(1)", Value::Int32(32768)),
            (
                "[Real]",
                "*CInt(2)",
                Value::Double(Double::from_float(f64::from(f32::MAX) * 2.0)),
            ),
        ] {
            let call = format!("{function}('{argument}','T','ID=1')");
            for sql in [
                format!("SELECT {call}{operation}"),
                format!("SELECT Q.N{operation} FROM (SELECT {call} AS N) AS Q"),
                format!("SELECT (SELECT Q.N{operation}) FROM (SELECT {call} AS N) AS Q"),
            ] {
                for declared in [false, true] {
                    assert_eq!(
                        run(&sql, &source, declared).unwrap(),
                        [vec![expected.clone()]],
                        "{sql}"
                    );
                }
            }
        }
    }
}

#[test]
fn all_numeric_domain_returns_supply_their_own_category() {
    let source = source();
    for (expression, expected) in [
        ("CInt(-32768)\\DAvg('CInt(-1)','T')", Value::Int32(32768)),
        (
            "CInt(-32768)\\DSum('CInt(-1)','T','ID=1')",
            Value::Int64(32768),
        ),
        ("CInt(7)\\DCount('*','T')", Value::Int64(3)),
        (
            "CInt(-32768)\\(DVar('CInt(1)','T')-CInt(1))",
            Value::Int32(32768),
        ),
        (
            "CInt(-32768)\\(DVarP('CInt(1)','T')-CInt(1))",
            Value::Int32(32768),
        ),
        (
            "CInt(-32768)\\(DStDev('CInt(1)','T')-CInt(1))",
            Value::Int32(32768),
        ),
        (
            "CInt(-32768)\\(DStDevP('CInt(1)','T')-CInt(1))",
            Value::Int32(32768),
        ),
    ] {
        let sql = format!("SELECT {expression}");
        for declared in [false, true] {
            assert_eq!(
                run(&sql, &source, declared).unwrap(),
                [vec![expected.clone()]],
                "{sql}"
            );
        }
    }
    for sql in [
        "SELECT CInt(DMin('Small','T'))+CInt(1)",
        "SELECT CInt(-32768)\\CDbl(DAvg('CInt(-1)','T'))",
    ] {
        assert_eq!(run(sql, &source, true), Err(EvalError::Overflow.into()));
    }
    assert_eq!(
        run("SELECT First(Small)+CInt(1) FROM T", &source, true),
        Err(QueryError::ArithmeticDeclaration)
    );
}

#[test]
fn domain_declarations_keep_null_error_data_and_operational_failures_distinct() {
    let source = source();
    let result = run(
        "SELECT DMin('Small','T','ID>2')+CInt(1),DAvg('Small','T','ID>2')+CInt(1),IsError(DLookup('CVErr(7)','T','ID=1'))",
        &source,
        true,
    ).unwrap();
    assert_eq!(
        result,
        [vec![Value::Null, Value::Null, Value::Boolean(true)]]
    );
    for (sql, expected) in [
        (
            "SELECT DLookup('CVErr(7)','T')+CInt(1)",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT DMin('Small','T',1/0)+CInt(1)",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT DSum('1/(ID-1)','T')+CInt(1)",
            EvalError::DivisionByZero,
        ),
    ] {
        assert_eq!(run(sql, &source, true), Err(expected.into()), "{sql}");
    }
    let parsed = parse_query("SELECT DLookup('CVErr(7)','T','ID=1')", Default::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute_expression(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), &[vec![ExpressionValue::Error(7)]]);
}
