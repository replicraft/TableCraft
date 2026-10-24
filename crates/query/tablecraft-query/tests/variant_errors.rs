use tablecraft_core::values::{Decimal, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[test]
fn error_consumers_keep_fields_grouping_aggregates_and_correlations() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT CLng(CVErr(A.N)),Count(*) FROM (SELECT 1 AS N UNION ALL SELECT 1) AS A GROUP BY CLng(CVErr(A.N))",
            vec![Value::Int32(1), Value::Int64(2)],
        ),
        (
            "SELECT CLng(CVErr(Sum(A.N))) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS A",
            vec![Value::Int32(3)],
        ),
        (
            "SELECT (SELECT IsError(CVErr(A.N))) FROM (SELECT 1 AS N) AS A",
            vec![Value::Boolean(true)],
        ),
        (
            "SELECT IsError(IIf(True,CVErr(2001),0)),IsError(Nz(Null,CVErr(2001))),IsError(Choose(2,0,CVErr(2001))),IsError(Switch(False,CVErr(1),True,CVErr(2)))",
            vec![Value::Boolean(true); 4],
        ),
        (
            "SELECT IIf(False,CVErr(2001),1),CVErr(0) IS NOT NULL",
            vec![Value::Int64(1), Value::Boolean(true)],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), [expected]);
    }
}

#[test]
fn decimal_error_conversion_is_explicit_across_derived_groups_and_correlations() {
    let source = MemorySource::default();
    for sql in [
        "SELECT CDec(CVErr(1))",
        "SELECT CDec(Q.Problem) FROM (SELECT CVErr(1) AS Problem) AS Q",
        "SELECT CDec(Min(Q.Problem)) FROM (SELECT CVErr(1) AS Problem) AS Q",
        "SELECT (SELECT CDec(Q.Problem)) FROM (SELECT CVErr(1) AS Problem) AS Q",
        "SELECT Q.ID FROM (SELECT 1 AS ID) AS Q WHERE CDec(CVErr(Q.ID))=1",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(QueryError::Evaluation(EvalError::TypeMismatch)),
            "{sql}"
        );
    }

    for sql in [
        "SELECT CDec(CLng(CVErr(2001)))",
        "SELECT CDec(CLng(Q.Problem)) FROM (SELECT CVErr(2001) AS Problem) AS Q",
        "SELECT CDec(CLng(Min(Q.Problem))) FROM (SELECT CVErr(2001) AS Problem) AS Q",
        "SELECT (SELECT CDec(CLng(Q.Problem))) FROM (SELECT CVErr(2001) AS Problem) AS Q",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Decimal(Decimal::new(2001, 0).unwrap())]]
        );
    }
}

#[test]
fn codes_are_execution_parameters_and_operational_errors_remain_failures() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT IsError(CVErr(P)),CLng(CVErr(P))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for code in [0, 2001, 65535] {
        let result = plan
            .execute(&source, &[("P", Value::Int32(code))], Default::default())
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Boolean(true), Value::Int32(code)]]
        );
    }
    assert_eq!(
        plan.execute(&source, &[("P", Value::Int32(-1))], Default::default())
            .err(),
        Some(EvalError::Domain.into())
    );
    for (sql, expected) in [
        ("SELECT CVErr(2001)", EvalError::NonScalarExpression),
        ("SELECT IsError(1/0)", EvalError::DivisionByZero),
        (
            "SELECT IsError(IIf(CVErr(1),0,1/0))",
            EvalError::DivisionByZero,
        ),
        ("SELECT IsError(IIf(CVErr(1),0,1))", EvalError::TypeMismatch),
        (
            "SELECT IsError(IIf(True,CVErr(2001),1/0))",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT IsError(Choose(1,CVErr(2001),1/0))",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT IsError(Switch(True,CVErr(2001),False,1/0))",
            EvalError::DivisionByZero,
        ),
        ("SELECT CInt(CVErr(65535))", EvalError::Overflow),
        (
            "SELECT A.N FROM (SELECT 1 AS N UNION ALL SELECT 2) AS A WHERE A.N=1 AND IsError(CVErr(IIf(A.N=1,1,-1)))",
            EvalError::Domain,
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(expected.into()),
            "{sql}"
        );
    }
}

#[test]
fn expression_selectors_preserve_existing_binary_and_null_array_views() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Payloads".into(),
            vec![Column::new("Payload".into(), ScalarType::Binary, false).unwrap()],
            vec![vec![Value::Binary(vec![1, 2])]],
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query("SELECT LBound(IIf(True,Payload,Payload)),UBound(Choose(1,Payload,Payload)),LBound(Nz(Payload,Payload)) FROM Payloads", Default::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], Default::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![Value::Int32(0), Value::Int32(1), Value::Int32(0)]]
    );
    let parsed = parse_query("SELECT LBound(IIf(True,Null,0))", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .err(),
        Some(EvalError::InvalidNullConversion.into())
    );
    let parsed = parse_query("SELECT IsArray(CVar(Array(1)))", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source).err(),
        Some(QueryError::Unsupported)
    );
}
