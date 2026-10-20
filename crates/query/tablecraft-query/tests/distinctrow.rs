use proptest::prelude::*;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, SchemaName};
use tablecraft_sql::{ParseLimits, parse_query};

fn table(name: &str, names: &[&str], rows: &[&[i32]]) -> MemoryTable {
    MemoryTable::new(
        name.into(),
        names
            .iter()
            .map(|name| Column::new((*name).into(), ScalarType::Int32, true).unwrap())
            .collect(),
        rows.iter()
            .map(|row| row.iter().map(|n| Value::Int32(*n)).collect())
            .collect(),
    )
    .unwrap()
}
fn source() -> MemorySource {
    MemorySource::new(vec![
        table(
            "A",
            &["Id", "Shown", "Hidden"],
            &[
                &[1, 7, 10],
                &[1, 7, 20],
                &[1, 7, 20],
                &[2, 8, 30],
                &[3, 9, 40],
            ],
        ),
        table("B", &["Owner", "Id"], &[&[1, 11], &[1, 12], &[2, 21]]),
    ])
    .unwrap()
}
fn run(sql: &str, source: &dyn Source) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}
#[test]
fn hidden_values_distinguish_records_but_identical_complete_records_collapse() {
    let sql = "SELECT DISTINCTROW A.Shown FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY Shown";
    assert_eq!(
        run(sql, &source()).unwrap(),
        [
            vec![Value::Int32(7)],
            vec![Value::Int32(7)],
            vec![Value::Int32(8)]
        ]
    );
    assert_eq!(
        run(&sql.replace("DISTINCTROW", "DISTINCT"), &source()).unwrap(),
        [vec![Value::Int32(7)], vec![Value::Int32(8)]]
    );
    assert_eq!(
        run(&sql.replace("DISTINCTROW ", ""), &source())
            .unwrap()
            .len(),
        7
    );
    assert_eq!(
        run(
            "SELECT DISTINCTROW A.Shown+1 AS V FROM A,B WHERE A.Id=B.Owner ORDER BY V",
            &source()
        )
        .unwrap(),
        [
            vec![Value::Int64(8)],
            vec![Value::Int64(8)],
            vec![Value::Int64(9)]
        ]
    );
}
#[test]
fn single_source_and_fields_from_every_source_ignore_distinctrow() {
    assert_eq!(
        run("SELECT DISTINCTROW Shown FROM A", &source())
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        run(
            "SELECT DISTINCTROW A.Shown,B.Owner FROM A,B WHERE A.Id=B.Owner",
            &source()
        )
        .unwrap()
        .len(),
        7
    );
    assert_eq!(
        run("SELECT DISTINCTROW Count(*) FROM A", &source()).unwrap(),
        [vec![Value::Int64(5)]]
    );
    assert_eq!(
        run("SELECT DISTINCTROW 1", &source()).unwrap(),
        [vec![Value::Int64(1)]]
    );
}
#[test]
fn aliases_nested_joins_and_null_padding_use_source_ranges() {
    assert_eq!(run("SELECT DISTINCTROW X.Shown FROM A AS X LEFT JOIN B AS Y ON X.Id=Y.Owner ORDER BY Shown",&source()).unwrap().len(),4);
    assert_eq!(run("SELECT DISTINCTROW X.Shown FROM B AS Y RIGHT JOIN A AS X ON X.Id=Y.Owner ORDER BY Shown",&source()).unwrap().len(),4);
    assert_eq!(run("SELECT DISTINCTROW X.Shown FROM A AS X INNER JOIN (B AS Y INNER JOIN B AS Z ON Y.Owner=Z.Owner) ON X.Id=Y.Owner ORDER BY Shown",&source()).unwrap().len(),3);
    assert_eq!(run("SELECT DISTINCTROW Y.Owner FROM A AS X LEFT JOIN B AS Y ON X.Id=Y.Owner ORDER BY Owner",&source()).unwrap(),[vec![Value::Null],vec![Value::Int32(1)],vec![Value::Int32(1)],vec![Value::Int32(2)]]);
}
#[test]
fn top_and_union_apply_after_record_deduplication() {
    assert_eq!(
        run(
            "SELECT DISTINCTROW TOP 1 A.Shown FROM A,B WHERE A.Id=B.Owner ORDER BY Shown",
            &source()
        )
        .unwrap()
        .len(),
        2
    );
    assert_eq!(
        run(
            "SELECT DISTINCTROW TOP 50 PERCENT A.Shown FROM A,B WHERE A.Id=B.Owner ORDER BY Shown",
            &source()
        )
        .unwrap()
        .len(),
        2
    );
    assert_eq!(
        run(
            "SELECT DISTINCTROW A.Shown FROM A,B WHERE A.Id=B.Owner UNION ALL SELECT 99",
            &source()
        )
        .unwrap()
        .len(),
        4
    );
    assert_eq!(
        run(
            "SELECT DISTINCTROW A.Shown FROM A,B WHERE A.Id=B.Owner UNION SELECT 99",
            &source()
        )
        .unwrap()
        .len(),
        3
    );
}
#[test]
fn unsupported_record_shapes_fail_during_metadata_only_binding() {
    struct Metadata {
        columns: Vec<Column>,
        unavailable: Vec<SchemaName>,
    }
    impl Source for Metadata {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
            Ok(&self.unavailable)
        }
        fn scan(
            &self,
            _: &str,
            _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("binding scanned records")
        }
    }
    let mut source = Metadata {
        columns: vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
        unavailable: vec![SchemaName::new("Files".into()).unwrap()],
    };
    let parsed = parse_query("SELECT DISTINCTROW A.Id FROM A,B", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source).err(),
        Some(QueryError::Unsupported)
    );
    source.unavailable.clear();
    assert!(prepare(parsed.query(), &source).is_ok());
    for sql in [
        "SELECT DISTINCTROW 1 FROM A,B",
        "SELECT DISTINCTROW A.Id,Count(*) FROM A,B GROUP BY A.Id",
        "SELECT DISTINCTROW A.Id FROM (SELECT Id FROM A) AS A,B",
        "SELECT DISTINCTROW A.Id FROM A,B ORDER BY B.Id",
        "SELECT DISTINCTROW A.Id,(SELECT 1) FROM A,B",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
}
#[test]
fn cancellation_limits_and_late_projection_errors_are_retained() {
    let source = source();
    let parsed = parse_query(
        "SELECT DISTINCTROW A.Shown FROM A,B",
        ParseLimits::default(),
    )
    .unwrap();
    let prepared = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        prepared
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| true),
                    ..Default::default()
                }
            )
            .err(),
        Some(QueryError::Cancelled)
    );
    assert!(
        prepared
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    limits: tablecraft_query::QueryLimits {
                        steps: 30,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(
        run(
            "SELECT DISTINCTROW A.Shown+1 \\ (A.Hidden-20) FROM A,B WHERE A.Id=B.Owner",
            &source
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}
#[test]
fn hidden_text_uses_explicit_full_value_collation() {
    struct Fold;
    impl Collation for Fold {
        fn compare(&self, a: &str, b: &str) -> Result<std::cmp::Ordering, EvalError> {
            Ok(a.to_lowercase().cmp(&b.to_lowercase()))
        }
    }
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "A".into(),
            vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Hidden".into(), ScalarType::Text, true).unwrap(),
            ],
            vec![
                vec![Value::Int32(1), Value::Text("a".into())],
                vec![Value::Int32(1), Value::Text("A".into())],
                vec![Value::Int32(1), Value::Null],
            ],
        )
        .unwrap(),
        table("B", &["Id"], &[&[1], &[2]]),
    ])
    .unwrap();
    let sql = "SELECT DISTINCTROW A.Id FROM A,B";
    assert_eq!(
        run(sql, &source).err(),
        Some(QueryError::Evaluation(EvalError::CollationRequired))
    );
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Fold),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows().len(), 2);
}
proptest! {
    #[test]
    fn generated_records_match_independent_full_record_semijoin(values in prop::collection::vec((0_i32..5,0_i32..3,0_i32..5),0..20), owners in prop::collection::vec(0_i32..5,0..12)) {
        let a=values.iter().map(|(id,shown,hidden)|vec![*id,*shown,*hidden]).collect::<Vec<_>>();
        let b=owners.iter().map(|id|vec![*id]).collect::<Vec<_>>();
        let source=MemorySource::new(vec![table("A",&["Id","Shown","Hidden"],&a.iter().map(Vec::as_slice).collect::<Vec<_>>()),table("B",&["Owner"],&b.iter().map(Vec::as_slice).collect::<Vec<_>>())]).unwrap();
        let expected=values.into_iter().filter(|(id,_,_)|owners.contains(id)).collect::<std::collections::BTreeSet<_>>().into_iter().map(|(_,shown,_)|shown).collect::<Vec<_>>();
        let mut expected=expected;expected.sort();
        let actual=run("SELECT DISTINCTROW A.Shown FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY Shown",&source).unwrap();
        prop_assert_eq!(actual,expected.into_iter().map(|n|vec![Value::Int32(n)]).collect::<Vec<_>>());
    }
}
