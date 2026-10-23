use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[test]
fn prepared_replacements_rebind_formatting_for_sources_and_inserted_scalars() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N DOUBLE; SELECT Replace(N,'.','X'),(SELECT Replace('x','x',N))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let bindings = [("N", Value::Double(Double::from_float(12.5)))];
    for (format, expected) in [
        (
            NumberFormat::EnUs,
            vec![Value::Text("12X5".into()), Value::Text("12.5".into())],
        ),
        (
            NumberFormat::DeDe,
            vec![Value::Text("12,5".into()), Value::Text("12,5".into())],
        ),
    ] {
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
            &[expected]
        );
    }
    assert!(
        plan.execute(&source, &bindings, Default::default())
            .is_err()
    );
}

#[test]
fn replacements_bind_group_and_correlate_scalar_source_fields() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![Column::new("N".into(), ScalarType::Int64, false).unwrap()],
            vec![vec![Value::Int64(123123)], vec![Value::Int64(111111)]],
        )
        .unwrap(),
    ])
    .unwrap();
    let options = ExecutionOptions {
        number_format: Some(NumberFormat::EnUs),
        ..Default::default()
    };
    let parsed = parse_query(
        "SELECT Len(Replace(N,23,45)) AS L,Count(*) FROM Items GROUP BY Len(Replace(N,23,45))",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options)
            .unwrap()
            .rows(),
        &[vec![Value::Int32(6), Value::Int64(2)]]
    );
    let parsed = parse_query(
        "SELECT (SELECT Replace(I.N,23,45)) FROM Items AS I",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options)
            .unwrap()
            .rows(),
        &[
            vec![Value::Text("145145".into())],
            vec![Value::Text("111111".into())]
        ]
    );
}

#[test]
fn unused_replacements_and_null_sources_do_not_hide_eager_errors() {
    let source = MemorySource::default();
    for sql in [
        "SELECT Replace(NULL,'a',1/0)",
        "SELECT Replace('','a',1/0)",
        "SELECT Replace('a','a',1/0,1,0)",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert!(matches!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default()),
            Err(QueryError::Evaluation(_))
        ));
    }
}

#[test]
fn replacement_states_preserve_option_and_string_conversion_order() {
    let source = MemorySource::default();
    for (sql, error) in [
        (
            "SELECT Replace(CVErr(1),'a',1/0)",
            EvalError::DivisionByZero,
        ),
        ("SELECT Replace(CVErr(1),'a','X',0)", EvalError::Domain),
        (
            "SELECT Replace('a','a','X',0,CVErr(1))",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT Replace(NULL,CVErr(1),'X')",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT Replace(123,CVErr(1),'X')",
            EvalError::NumberFormatRequired,
        ),
        ("SELECT Replace('',CVErr(1),'X')", EvalError::TypeMismatch),
        ("SELECT Replace(CVErr(1),'a','X',1,-1,3)", EvalError::Domain),
        (
            "SELECT Replace(Q.E,'a',1/(Q.N-1)) FROM (SELECT CVErr(1) AS E,1 AS N) AS Q",
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
}

#[test]
fn replacement_states_compose_with_explicit_cstr_and_encounter_aggregates() {
    let source = MemorySource::default();
    for sql in [
        "SELECT Replace(CStr(CVErr(11)),'11','5')",
        "SELECT Replace(CStr(First(CVErr(Q.N))),'1','5') FROM (SELECT 1 AS N) AS Q",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Text("Error 5".into())]],
            "{sql}"
        );
    }
}
