use tablecraft_core::values::{Decimal, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Unused".into(), ScalarType::Text, false).unwrap(),
                Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
            ],
            [-2, 0, 5]
                .into_iter()
                .enumerate()
                .map(|(i, n)| {
                    vec![
                        Value::Int32(i as i32 + 1),
                        Value::Text("unused".into()),
                        Value::Int32(n),
                    ]
                })
                .chain(std::iter::once(vec![
                    Value::Int32(4),
                    Value::Text("unused".into()),
                    Value::Null,
                ]))
                .collect(),
        )
        .unwrap(),
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
fn selections_prune_rewrite_and_preserve_null_and_mixed_carriers() {
    let source = source();
    assert_eq!(run("SELECT Switch(Score<0,CInt(-1),Score=0,CInt(0),Score>0,CInt(1),True,CInt(9)) AS K FROM T ORDER BY Id",&source).unwrap(),[vec![Value::Int16(-1)],vec![Value::Int16(0)],vec![Value::Int16(1)],vec![Value::Int16(9)]]);
    assert_eq!(
        run(
            "SELECT Choose(Id,CInt(7),'two',CDec(1.25),NULL) FROM T ORDER BY Id",
            &source
        )
        .unwrap(),
        [
            vec![Value::Int16(7)],
            vec![Value::Text("two".into())],
            vec![Value::Decimal(Decimal::new(125, 2).unwrap())],
            vec![Value::Null]
        ]
    );
    assert_eq!(
        run(
            "SELECT Switch(Score<0,NULL,True,Id) FROM T ORDER BY Id",
            &source
        )
        .unwrap(),
        [
            vec![Value::Null],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)],
            vec![Value::Int32(4)]
        ]
    );
}
#[test]
fn grouping_aggregates_correlations_and_crosstab_cells_visit_all_operands() {
    let source = source();
    assert_eq!(run("SELECT Switch(Score<0,1,True,2) AS K,Count(*) FROM T GROUP BY Switch(Score<0,1,True,2) ORDER BY K",&source).unwrap(),[vec![Value::Int64(1),Value::Int64(1)],vec![Value::Int64(2),Value::Int64(3)]]);
    assert_eq!(
        run("SELECT Choose(1,Sum(Score),Count(*)) FROM T", &source).unwrap(),
        [vec![Value::Int64(3)]]
    );
    assert_eq!(
        run(
            "SELECT (SELECT Choose(1,A.Score,99)) FROM T AS A ORDER BY Id",
            &source
        )
        .unwrap(),
        [
            vec![Value::Int32(-2)],
            vec![Value::Int32(0)],
            vec![Value::Int32(5)],
            vec![Value::Null]
        ]
    );
    assert_eq!(run("TRANSFORM Choose(1,Sum(Score),Count(*)) SELECT 1 AS R FROM T GROUP BY 1 PIVOT Choose(Id,1,1,2,2) IN (1,2)",&source).unwrap(),[vec![Value::Int64(1),Value::Int64(-2),Value::Int64(5)]]);
    assert_eq!(
        run("SELECT Choose(1,Score,Id) FROM T GROUP BY Score", &source).err(),
        Some(QueryError::Grouping)
    );
}
#[test]
fn parameters_are_rebound_without_retaining_old_choice_or_carrier() {
    let source = source();
    let parsed = parse_query(
        "PARAMETERS Pick LONG; SELECT Choose(Pick,Score,CInt(7)) FROM T ORDER BY Id",
        ParseLimits::default(),
    )
    .unwrap();
    let prepared = prepare(parsed.query(), &source).unwrap();
    for (pick, first) in [
        (1, Value::Int32(-2)),
        (2, Value::Int16(7)),
        (3, Value::Null),
    ] {
        let result = prepared
            .execute(
                &source,
                &[("Pick", Value::Int32(pick))],
                ExecutionOptions::default(),
            )
            .unwrap();
        assert_eq!(result.rows()[0], [first]);
    }
}
#[test]
fn bad_arities_and_missing_names_fail_during_metadata_binding() {
    struct Metadata(Vec<Column>);
    impl Source for Metadata {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.0)
        }
        fn scan(
            &self,
            _: &str,
            _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("binding scanned records")
        }
    }
    let source = Metadata(vec![
        Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
    ]);
    for sql in [
        "SELECT Switch() FROM T",
        "SELECT Switch(True,1,False) FROM T",
        "SELECT Choose(1) FROM T",
        "SELECT Choose(1,,3) FROM T",
        "SELECT VBA.Choose(1,Id) FROM T",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
    let parsed = parse_query("SELECT Choose(1,Id,Missing) FROM T", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source).err(),
        Some(QueryError::UnknownName)
    );
    let parsed = parse_query(
        "SELECT Switch(True,Id,False,Id) FROM T",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(prepare(parsed.query(), &source).is_ok());
}
#[test]
fn later_expression_errors_and_cancelled_unselected_branches_are_not_suppressed() {
    let source = source();
    for sql in [
        "SELECT Switch(True,1,False,1/0) FROM T",
        "SELECT Choose(NULL,1/0) FROM T",
        "SELECT Choose('bad',1,1/0) FROM T",
        "SELECT Switch('bad',1,True,1/0) FROM T",
        "SELECT Choose(99,1,1/0) FROM T",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
    let query = parse_query(
        "SELECT Switch(True,1,'bad',2) FROM T",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (profile, error) in [
        (None, EvalError::NumberFormatRequired),
        (Some(NumberFormat::EnUs), EvalError::TypeMismatch),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: profile,
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::Evaluation(error))
        );
    }
    let parsed = parse_query("SELECT Choose(1,Id,Score) FROM T", ParseLimits::default()).unwrap();
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
                        steps: 10,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            )
            .is_err()
    );
}
