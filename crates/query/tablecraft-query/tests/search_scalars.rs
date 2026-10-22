use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[test]
fn prepared_searches_rebind_number_policies_without_caching_spellings() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N DOUBLE; SELECT InStr(1,N,'.',0),InStrRev(N,','),(SELECT InStrRev(N,'.'))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let bindings = [("N", Value::Double(Double::from_float(12.5)))];
    for (format, expected) in [
        (
            NumberFormat::EnUs,
            vec![Value::Int32(3), Value::Int32(0), Value::Int32(3)],
        ),
        (
            NumberFormat::DeDe,
            vec![Value::Int32(0), Value::Int32(3), Value::Int32(0)],
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
fn scalar_searches_group_filter_and_correlate_native_carriers() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![Column::new("N".into(), ScalarType::Int64, true).unwrap()],
            vec![
                vec![Value::Int64(123123)],
                vec![Value::Int64(111111)],
                vec![Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let options = ExecutionOptions {
        number_format: Some(NumberFormat::EnUs),
        ..Default::default()
    };
    let parsed = parse_query(
        "SELECT InStr(1,N,23,0) AS P,Count(*) FROM Items GROUP BY InStr(1,N,23,0) ORDER BY P",
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
            vec![Value::Null, Value::Int64(1)],
            vec![Value::Int32(0), Value::Int64(1)],
            vec![Value::Int32(2), Value::Int64(1)]
        ]
    );
    let parsed = parse_query(
        "SELECT (SELECT InStrRev(I.N,23)) FROM Items AS I WHERE InStr(1,I.N,23,0)>0",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options)
            .unwrap()
            .rows(),
        &[vec![Value::Int32(5)]]
    );
}

#[test]
fn null_does_not_hide_eager_argument_failures() {
    let source = MemorySource::default();
    for sql in ["SELECT InStr(1,NULL,1/0,0)", "SELECT InStrRev(NULL,1/0)"] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert!(matches!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default()),
            Err(QueryError::Evaluation(_))
        ));
    }
}
