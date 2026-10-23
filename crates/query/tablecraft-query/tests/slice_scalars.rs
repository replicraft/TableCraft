use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn prepared_scalar_slices_rebind_formats_and_preserve_nested_counts() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N DOUBLE; SELECT Left(N,2),Right(N,2),(SELECT Mid(N,3)),Mid(N,'2,5','2,5')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let bindings = [("N", Value::Double(Double::from_float(12.5)))];
    let result = plan
        .execute(
            &source,
            &bindings,
            ExecutionOptions {
                number_format: Some(NumberFormat::DeDe),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            Value::Text("12".into()),
            Value::Text(",5".into()),
            Value::Text(",5".into()),
            Value::Text("2,".into())
        ]]
    );
    let parsed = parse_query("PARAMETERS N DOUBLE; SELECT Right(N,2)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (format, expected) in [(NumberFormat::EnUs, ".5"), (NumberFormat::DeDe, ",5")] {
        assert_eq!(
            plan.execute(
                &source,
                &bindings,
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            &[vec![Value::Text(expected.into())]]
        );
    }
    assert!(
        plan.execute(&source, &bindings, Default::default())
            .is_err()
    );
}

#[test]
fn null_sources_preserve_eager_argument_failures_and_no_count_conversion() {
    let source = MemorySource::default();
    for function in ["Left", "Right", "Mid"] {
        let parsed =
            parse_query(&format!("SELECT {function}(NULL,1/0)"), Default::default()).unwrap();
        assert!(matches!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default()),
            Err(QueryError::Evaluation(_))
        ));
        let parsed = parse_query(
            &format!("SELECT {function}(NULL,'bad')"),
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Null]]
        );
    }
}

#[test]
fn slice_states_keep_later_operational_failures_eager_and_admission_progressive() {
    let source = MemorySource::default();
    for (sql, error) in [
        ("SELECT Left(CVErr(1),1/0)", EvalError::DivisionByZero),
        ("SELECT Right(CVErr(1),1/0)", EvalError::DivisionByZero),
        ("SELECT Mid('abc',CVErr(1),1/0)", EvalError::DivisionByZero),
        ("SELECT Mid('abc',0,CVErr(1))", EvalError::Domain),
        ("SELECT Mid('abc',1,CVErr(1))", EvalError::TypeMismatch),
        ("SELECT Left(123,CVErr(1))", EvalError::NumberFormatRequired),
        (
            "SELECT Left(Q.E,1/(Q.N-1)) FROM (SELECT CVErr(1) AS E,1 AS N) AS Q",
            EvalError::DivisionByZero,
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(error.into()),
            "{sql}"
        );
    }
    let parsed = parse_query(
        "SELECT Left(NULL,CVErr(1)),Right(NULL,CVErr(1)),Mid(NULL,0,CVErr(1))",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Null, Value::Null, Value::Null]]
    );
}

#[test]
fn slice_plans_reuse_after_source_state_failure_and_keep_explicit_string_conversion() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N LONG; SELECT Left(IIf(N=0,CVErr(1),'abc'),2),Right(CStr(CVErr(11)),2)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[("N", Value::Int32(0))], Default::default())
            .err(),
        Some(EvalError::TypeMismatch.into())
    );
    assert_eq!(
        plan.execute(&source, &[("N", Value::Int32(1))], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Text("ab".into()), Value::Text("11".into())]]
    );
}
