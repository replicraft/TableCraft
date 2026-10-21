use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

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
