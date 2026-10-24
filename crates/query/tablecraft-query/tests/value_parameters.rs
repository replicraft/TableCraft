use std::cell::Cell;
use tablecraft_core::values::{
    Currency, Decimal, Double, ExtendedDateTime, Guid, LegacyDateTime, Single, Value,
};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, QueryLimits, Source, prepare, source::MemorySource,
};
use tablecraft_schema::Column;
use tablecraft_sql::{parse_query, query::Query};

#[test]
fn value_parameters_rebind_exact_scalar_carriers_without_freezing_metadata() {
    let source = MemorySource::default();
    for sql in [
        "PARAMETERS P VALUE; SELECT P AS V",
        "PARAMETERS P VALUE; SELECT Q.V FROM (SELECT P AS V) AS Q",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(plan.output_types(), Some([OutputType::Dynamic].as_slice()));

        for value in [
            Value::Null,
            Value::Boolean(true),
            Value::Byte(255),
            Value::Int16(-32768),
            Value::Int32(2_147_483_647),
            Value::Int64(i64::MIN),
            Value::Single(Single::from_bits(0x8000_0000)),
            Value::Double(Double::from_bits(0x3ff0_0000_0000_0001)),
            Value::Currency(Currency::from_scaled(-12345)),
            Value::Decimal(Decimal::new(12300, 4).unwrap()),
            Value::Text("é🦀; SQL stays data".into()),
            Value::Binary(vec![0, 255, 1]),
            Value::Guid(Guid::from_bytes([0x12; 16])),
            Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
            Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 24, 1, 2, 3, 4).unwrap()),
        ] {
            let result = plan
                .execute(&source, &[("p", value.clone())], Default::default())
                .unwrap();
            assert_eq!(result.rows(), &[vec![value]]);
            assert_eq!(plan.output_types(), Some([OutputType::Dynamic].as_slice()));
        }
    }
}

#[test]
fn value_declarations_retain_variant_widening_and_typed_parameters_remain_strict() {
    let source = MemorySource::default();
    for (data_type, expected) in [
        ("VALUE", Ok(vec![vec![Value::Int32(32768)]])),
        ("SHORT", Err(QueryError::Evaluation(EvalError::Overflow))),
    ] {
        for body in [
            "SELECT P+CInt(1)",
            "SELECT Q.V+CInt(1) FROM (SELECT P AS V) AS Q",
            "SELECT (SELECT P)+CInt(1)",
            "SELECT P+CInt(1) GROUP BY P",
        ] {
            let parsed = parse_query(
                &format!("PARAMETERS P {data_type}; {body}"),
                Default::default(),
            )
            .unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            let result = plan.execute(
                &source,
                &[("P", Value::Int16(32767))],
                ExecutionOptions {
                    declared_integral: true,
                    ..Default::default()
                },
            );
            assert_eq!(
                result.map(|result| result.rows().to_vec()),
                expected,
                "{body}"
            );
        }
    }

    let parsed = parse_query("PARAMETERS P LONG; SELECT P", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[("P", Value::Int16(1))], Default::default())
            .err(),
        Some(QueryError::Parameter)
    );
}

#[test]
fn value_parameter_payloads_share_budgets_and_every_cancellation_checkpoint() {
    let source = MemorySource::default();
    let parsed = parse_query("PARAMETERS P VALUE; SELECT P AS V", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let parameters = [("P", Value::Binary(vec![7; 256]))];
    assert!(matches!(
        plan.execute(
            &source,
            &parameters,
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 128,
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
        Err(QueryError::Limit(_))
    ));

    let calls = Cell::new(0);
    plan.execute(
        &source,
        &parameters,
        ExecutionOptions {
            cancelled: Some(&|| {
                calls.set(calls.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(calls.get() > 0);
    for stop in 1..=calls.get() {
        let current = Cell::new(0);
        assert_eq!(
            plan.execute(
                &source,
                &parameters,
                ExecutionOptions {
                    cancelled: Some(&|| {
                        current.set(current.get() + 1);
                        current.get() == stop
                    }),
                    ..Default::default()
                },
            )
            .err(),
            Some(QueryError::Cancelled),
            "checkpoint {stop}"
        );
    }
    assert_eq!(
        plan.execute(&source, &parameters, Default::default())
            .unwrap()
            .rows(),
        &[vec![parameters[0].1.clone()]]
    );
}

#[test]
fn value_parameters_keep_name_finiteness_and_text_limits() {
    let source = MemorySource::default();
    let parsed = parse_query("PARAMETERS P VALUE; SELECT P", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let mut invalid = parsed.query().clone();
    invalid.parameters[0].data_type.arguments.push(1);
    assert_eq!(
        prepare(&invalid, &source).err(),
        Some(QueryError::Unsupported)
    );

    for parameters in [
        vec![],
        vec![("Q", Value::Int32(1))],
        vec![("P", Value::Int32(1)), ("p", Value::Int32(2))],
        vec![("P", Value::Int32(1)), ("Extra", Value::Null)],
        vec![("P", Value::Single(Single::from_bits(0x7f80_0000)))],
        vec![("P", Value::Double(Double::from_bits(0x7ff8_0000_0000_0001)))],
    ] {
        assert_eq!(
            plan.execute(&source, &parameters, Default::default()).err(),
            Some(QueryError::Parameter)
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("P", Value::Text("abc".into()))],
            ExecutionOptions {
                max_text_bytes: Some(2),
                ..Default::default()
            },
        )
        .err(),
        Some(QueryError::Parameter)
    );

    assert_eq!(
        plan.execute(&source, &[("P", Value::Int32(1))], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Int32(1)]]
    );
}

struct NamedValues {
    first: Query,
    second: Query,
}

impl Source for NamedValues {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(match name {
            "FirstValues" => Some(&self.first),
            "SecondValues" => Some(&self.second),
            _ => None,
        })
    }

    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Err(QueryError::UnknownName)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("source-free named queries must not scan tables")
    }
}

#[test]
fn named_value_parameters_share_compatible_scopes_and_refuse_typed_conflicts() {
    let query = |sql| parse_query(sql, Default::default()).unwrap().into_query();
    let mut source = NamedValues {
        first: query("PARAMETERS P VALUE; SELECT P AS V"),
        second: query("PARAMETERS p VALUE; SELECT p AS V"),
    };
    let parsed = query("SELECT A.V,B.V FROM FirstValues AS A,SecondValues AS B");
    let plan = prepare(&parsed, &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[("P", Value::Byte(7))], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Byte(7), Value::Byte(7)]]
    );
    source.second = query("PARAMETERS P BYTE; SELECT P AS V");
    assert_eq!(prepare(&parsed, &source).err(), Some(QueryError::Parameter));
}
