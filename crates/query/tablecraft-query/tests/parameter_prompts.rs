use tablecraft_core::values::Value;
use tablecraft_query::{
    QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[test]
fn bracketed_prompt_and_dotted_field_remain_distinct_with_checked_reusable_parameters() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "P".into(),
            vec![Column::new("Q".into(), ScalarType::Int64, false).unwrap()],
            vec![vec![Value::Int64(9)]],
        )
        .unwrap(),
    ])
    .unwrap();
    let query = parse_query(
        "PARAMETERS [P.Q] LONG; SELECT [P.Q] AS Prompt,P.Q AS FieldValue FROM P",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for value in [1, 2] {
        assert_eq!(
            plan.execute(&source, &[("p.q", Value::Int32(value))], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Int32(value), Value::Int64(9)]]
        );
    }
    for values in [
        vec![],
        vec![("P.Q", Value::Int64(1))],
        vec![("P.Q", Value::Int32(1)), ("p.q", Value::Int32(2))],
    ] {
        assert_eq!(
            plan.execute(&source, &values, Default::default()).err(),
            Some(QueryError::Parameter)
        );
    }
    let missing = parse_query("SELECT [Enter No. of items]", Default::default()).unwrap();
    assert_eq!(
        prepare(missing.query(), &source).err(),
        Some(QueryError::UnknownName)
    );
}

#[test]
fn literal_prompt_punctuation_stays_literal_in_nested_reads() {
    let source = MemorySource::new(vec![]).unwrap();
    for prompt in [
        "Enter No. of items",
        "Floor!",
        "Forms!F!C",
        "Rate `value`",
        "P) OR (True; --",
    ] {
        let query = parse_query(
            &format!("PARAMETERS [{prompt}] LONG; SELECT (SELECT [{prompt}]) AS Result"),
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[(prompt, Value::Int32(7))], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Int32(7)]]
        );
    }
    let control = parse_query("SELECT Forms!F!C", Default::default()).unwrap();
    assert_eq!(
        prepare(control.query(), &source).err(),
        Some(QueryError::Unsupported)
    );
}

#[test]
fn field_resolution_still_precedes_an_identically_spelled_parameter() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("P.Q".into(), ScalarType::Int64, false).unwrap()],
            vec![vec![Value::Int64(9)]],
        )
        .unwrap(),
    ])
    .unwrap();
    let query = parse_query(
        "PARAMETERS [P.Q] LONG; SELECT [P.Q] FROM T",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[("P.Q", Value::Int32(2))], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Int64(9)]]
    );
}
