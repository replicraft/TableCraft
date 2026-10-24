use super::{
    raw_support::{RawResolver, Units},
    support::*,
};
use tablecraft_query::source::MemorySource;

#[test]
fn raw_strcomp_uses_database_default_while_predicates_keep_the_field_policy() {
    let source = Fixture::new();
    let parsed = parse_query(
        "SELECT StrComp(A&ChrW(55296),'A'&ChrW(55296),2),(A&ChrW(55296))=('A'&ChrW(55296)),StrComp(U&ChrW(55296),'x',2) FROM T WHERE Id=1",
        Default::default(),
    ).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Units(false)),
                collation_resolver: Some(&RawResolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            Value::Int16(1),
            Value::Boolean(true),
            Value::Int16(-1)
        ]]
    );
}

#[test]
fn raw_database_strcomp_composes_with_parameters_aggregates_and_byte_views() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT StrComp(ChrW(P),ChrW(P),2)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let options = ExecutionOptions {
        collation: Some(&Units(false)),
        ..Default::default()
    };
    for code in [65, 55296, 57343] {
        let result = plan
            .execute(&source, &[("P", Value::Int32(code))], options)
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Int16(0)]]);
    }

    for sql in [
        "SELECT StrComp(MidB('😀',1,2),ChrW(55357),2)",
        "SELECT StrComp(Min(S),ChrW(55296),2) FROM (SELECT ChrW(55296) AS S UNION ALL SELECT ChrW(55297)) AS Q",
        "SELECT StrComp(S,ChrW(55296),2) FROM (SELECT ChrW(55296) AS S) AS Q",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options)
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Int16(0)]], "{sql}");
    }
}
