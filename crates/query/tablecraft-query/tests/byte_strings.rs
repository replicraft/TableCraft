use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn byte_states_preserve_derived_errors_until_control_and_source_admission() {
    for (sql, expected) in [
        ("SELECT LenB(CVErr(11))", EvalError::TypeMismatch),
        ("SELECT AscB(CVErr(11))", EvalError::TypeMismatch),
        ("SELECT ChrB(CVErr(65))", EvalError::TypeMismatch),
        (
            "SELECT LeftB(E,-1) FROM (SELECT CVErr(11) AS E) AS D",
            EvalError::Domain,
        ),
        (
            "SELECT MidB(CVErr(11),0,CVErr(12))",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT InStrB(CVErr(11),NULL,CVErr(12),3)",
            EvalError::UnsupportedExpression,
        ),
        (
            "SELECT InStrB(1,123,CVErr(11),0)",
            EvalError::NumberFormatRequired,
        ),
        ("SELECT LeftB(CVErr(11),1/0)", EvalError::DivisionByZero),
    ] {
        assert_eq!(run(sql), Err(QueryError::Evaluation(expected)), "{sql}");
    }
    assert_eq!(
        run("SELECT InStrB(1,NULL,CVErr(11),0),LenB(LeftB(CStr(CVErr(11)),3)),AscB(ChrB(CLng(CVErr(65))))").unwrap(),
        [vec![Value::Null, Value::Int32(3), Value::Int16(65)]]
    );
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn byte_observations_retain_fields_filters_groups_and_correlated_scopes() {
    assert_eq!(
        run("SELECT LenB(A.T),(SELECT AscB(A.T)) FROM (SELECT 'é中🦀' AS T UNION ALL SELECT 'Ā') AS A ORDER BY 1").unwrap(),
        [vec![Value::Int32(2), Value::Int16(0)], vec![Value::Int32(8), Value::Int16(233)]]
    );
    assert_eq!(
        run("SELECT LenB(A.T),Count(*) FROM (SELECT 'é' AS T UNION ALL SELECT '中') AS A GROUP BY LenB(A.T)").unwrap(),
        [vec![Value::Int32(2), Value::Int64(2)]]
    );
    assert_eq!(
        run("SELECT Sum(LenB(A.T)) FROM (SELECT 'é中🦀' AS T UNION ALL SELECT 'Ā') AS A WHERE AscB(A.T)>0").unwrap(),
        [vec![Value::Int64(8)]]
    );
    assert_eq!(
        run("SELECT LenB(A.T) FROM (SELECT 'é' AS T) AS A GROUP BY Len(A.T)").err(),
        Some(QueryError::Grouping)
    );
}

#[test]
fn byte_functions_keep_parameter_subtypes_nulls_and_eager_errors() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P TEXT; SELECT LenB(P),AscB(P)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (text, bytes, first) in [("é", 2, 233), ("🦀", 4, 62)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Text(text.into()))],
                Default::default()
            )
            .unwrap()
            .rows(),
            [vec![Value::Int32(bytes), Value::Int16(first)]]
        );
    }
    assert_eq!(run("SELECT LenB(Null)").unwrap(), [vec![Value::Null]]);
    assert_eq!(
        run("SELECT AscB(Null)").err(),
        Some(EvalError::InvalidNullConversion.into())
    );
    for sql in ["SELECT LenB(1/0)", "SELECT IIf(False,AscB(1/0),0)"] {
        assert_eq!(run(sql).err(), Some(EvalError::DivisionByZero.into()));
    }
    for sql in [
        "SELECT LenB(*)",
        "SELECT AscB(,)",
        "SELECT LenB('x',1/0)",
        "SELECT VBA.AscB('x')",
    ] {
        assert_eq!(run(sql).err(), Some(QueryError::Unsupported));
    }
}

#[test]
fn raw_byte_intermediates_keep_group_identity_aggregate_dependencies_and_scopes() {
    assert_eq!(
        run("SELECT AscB(LeftB(A.T,1)),Count(*) FROM (SELECT 'é' AS T UNION ALL SELECT 'é中') AS A GROUP BY AscB(LeftB(A.T,1))").unwrap(),
        [vec![Value::Int16(233), Value::Int64(2)]]
    );
    assert_eq!(
        run("SELECT (SELECT AscB(RightB(A.T,3))) FROM (SELECT '🦀' AS T) AS A").unwrap(),
        [vec![Value::Int16(216)]]
    );
    assert_eq!(
        run("SELECT AscB(ChrB(Sum(A.N))) FROM (SELECT 65 AS N UNION ALL SELECT 66) AS A").unwrap(),
        [vec![Value::Int16(131)]]
    );
    assert_eq!(
        run("SELECT LenB(LeftB(A.T,1)) FROM (SELECT 'A' AS T) AS A GROUP BY LenB(RightB(A.T,1))")
            .err(),
        Some(QueryError::Grouping)
    );
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P TEXT; SELECT AscB(MidB(P,2,1)),LenB(CVar(LeftB(P,1)))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (text, byte) in [("中", 78), ("🦀", 216)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Text(text.into()))],
                Default::default()
            )
            .unwrap()
            .rows(),
            [vec![Value::Int16(byte), Value::Int32(1)]]
        );
    }
}

#[test]
fn raw_byte_calls_preserve_string_inspection_and_strict_final_publication() {
    let result = run(
        "SELECT TypeName(ChrB(65)),VarType(CVar(ChrB(65))),IsArray(ChrB(65)),LenB(LeftB(Null,1)),MidB('A中🦀',3,6)",
    ).unwrap();
    assert_eq!(
        result,
        [vec![
            Value::Text("String".into()),
            Value::Int16(8),
            Value::Boolean(false),
            Value::Null,
            Value::Text("中🦀".into()),
        ]]
    );
    for sql in [
        "SELECT ChrB(65)",
        "SELECT CVar(ChrB(65))",
        "SELECT LeftB('A',1)",
        "SELECT RightB('🦀',2)",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::UnrepresentableText.into()),
            "{sql}"
        );
    }
    for sql in [
        "SELECT LenB(LeftB(Null,1/0))",
        "SELECT LenB(MidB('A',0,1/0))",
    ] {
        assert_eq!(run(sql).err(), Some(EvalError::DivisionByZero.into()));
    }
    for sql in [
        "SELECT ChrB(1,1/0)",
        "SELECT MidB('x',1,)",
        "SELECT LeftB(*)",
    ] {
        assert_eq!(run(sql).err(), Some(QueryError::Unsupported));
    }
    assert_eq!(
        run("SELECT LBound(ChrB(65))").err(),
        Some(EvalError::TypeMismatch.into())
    );
}

#[test]
fn raw_byte_query_evaluation_checks_every_observed_cancellation_point() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT MidB('A中🦀',3,6),LenB(CVar(RightB('🦀',3))),InStrB(1,'A中🦀',ChrB(216),0)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    let count = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    let total = checkpoints.get();
    for stop in 1..=total {
        checkpoints.set(0);
        let cancel = || {
            checkpoints.set(checkpoints.get() + 1);
            checkpoints.get() == stop
        };
        assert!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancel),
                    ..Default::default()
                }
            )
            .is_err(),
            "checkpoint {stop}"
        );
    }
}

#[test]
fn instrb_retains_scopes_group_identity_and_aggregate_dependencies() {
    let grouped = run(
        "SELECT InStrB(1,A.T,ChrB(0),0),Count(*) FROM (SELECT 'A' AS T UNION ALL SELECT 'B') AS A GROUP BY InStrB(1,A.T,ChrB(0),0)",
    ).unwrap();
    assert_eq!(grouped, [vec![Value::Int32(2), Value::Int64(2)]]);
    assert_eq!(
        run("SELECT (SELECT InStrB(1,A.T,ChrB(216),0)) FROM (SELECT '🦀' AS T) AS A").unwrap(),
        [vec![Value::Int32(2)]]
    );
    assert_eq!(
        run("SELECT InStrB(1,'A',ChrB(Sum(A.N)),0) FROM (SELECT 65 AS N) AS A").unwrap(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(run("SELECT InStrB(1,A.T,ChrB(0),0) FROM (SELECT 'A' AS T) AS A GROUP BY InStrB(2,A.T,ChrB(0),0)").err(), Some(QueryError::Grouping));
    for sql in [
        "SELECT InStrB(*)",
        "SELECT InStrB('A','A',0,0,1/0)",
        "SELECT InStrB(1,,'A',0)",
    ] {
        assert_eq!(run(sql).err(), Some(QueryError::Unsupported));
    }
}

#[path = "byte_strings/output_types.rs"]
mod output_types;
