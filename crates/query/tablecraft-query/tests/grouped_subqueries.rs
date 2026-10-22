use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Bucket".into(), ScalarType::Int32, false).unwrap(),
            ],
            [(1, 1), (2, 1), (3, 2), (4, 2), (5, 3)]
                .into_iter()
                .map(|(id, bucket)| vec![Value::Int32(id), Value::Int32(bucket)])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source();
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn grouped_scalar_and_having_subqueries_capture_only_group_keys() {
    let rows = [
        vec![Value::Int32(1), Value::Int64(2)],
        vec![Value::Int32(2), Value::Int64(2)],
        vec![Value::Int32(3), Value::Int64(1)],
    ];
    for sql in [
        "SELECT O.Bucket,(SELECT Count(*) FROM Items I WHERE I.Bucket=O.Bucket) FROM Items O GROUP BY O.Bucket ORDER BY O.Bucket",
        "SELECT O.Bucket,Count(*) FROM Items O GROUP BY O.Bucket HAVING Count(*) >= (SELECT Count(*) FROM Items I WHERE I.Bucket=O.Bucket) ORDER BY O.Bucket",
        "SELECT O.Bucket,(SELECT Count(*) FROM Items I INNER JOIN Items J ON I.Id=J.Id AND J.Bucket=O.Bucket) FROM Items O GROUP BY O.Bucket ORDER BY O.Bucket",
    ] {
        assert_eq!(run(sql).unwrap(), rows, "{sql}");
    }
}

#[test]
fn exists_membership_quantified_and_union_subqueries_work_after_grouping() {
    let all = [
        vec![Value::Int32(1)],
        vec![Value::Int32(2)],
        vec![Value::Int32(3)],
    ];
    for sql in [
        "SELECT O.Bucket FROM Items O GROUP BY O.Bucket HAVING O.Bucket >= ALL(SELECT I.Bucket FROM Items I WHERE I.Id<=2) ORDER BY O.Bucket",
        "SELECT O.Bucket FROM Items O GROUP BY O.Bucket HAVING O.Bucket IN(SELECT O.Bucket UNION ALL SELECT 4) ORDER BY O.Bucket",
    ] {
        assert_eq!(run(sql).unwrap(), all, "{sql}");
    }
    for sql in [
        "SELECT O.Bucket FROM Items O GROUP BY O.Bucket HAVING EXISTS(SELECT I.Id FROM Items I WHERE I.Bucket=O.Bucket AND I.Id>3) ORDER BY O.Bucket",
        "SELECT O.Bucket FROM Items O GROUP BY O.Bucket HAVING O.Bucket IN(SELECT I.Bucket FROM Items I WHERE I.Id>3) ORDER BY O.Bucket",
    ] {
        assert_eq!(run(sql).unwrap(), &all[1..], "{sql}");
    }
    assert_eq!(run("SELECT O.Bucket FROM Items O GROUP BY O.Bucket HAVING O.Bucket=ANY(SELECT I.Bucket FROM Items I WHERE I.Id<=2)").unwrap(), &all[..1]);
}

#[test]
fn computed_keys_and_multiple_query_frames_keep_their_bound_identity() {
    assert_eq!(run("SELECT O.Bucket Mod 2 AS K,(SELECT Count(*) FROM Items I WHERE I.Bucket Mod 2=O.Bucket Mod 2) FROM Items O GROUP BY O.Bucket Mod 2 ORDER BY K").unwrap(), [vec![Value::Int64(0),Value::Int64(2)],vec![Value::Int64(1),Value::Int64(3)]]);
    assert_eq!(run("SELECT O.Bucket,(SELECT (SELECT O.Bucket)) FROM Items O GROUP BY O.Bucket ORDER BY O.Bucket").unwrap(), [vec![Value::Int32(1),Value::Int32(1)],vec![Value::Int32(2),Value::Int32(2)],vec![Value::Int32(3),Value::Int32(3)]]);
    assert_eq!(run("SELECT O.Bucket,(SELECT Sum(O.Bucket) FROM Items I) FROM Items O GROUP BY O.Bucket ORDER BY O.Bucket").unwrap(), [vec![Value::Int32(1),Value::Int64(5)],vec![Value::Int32(2),Value::Int64(10)],vec![Value::Int32(3),Value::Int64(15)]]);
}

struct MetadataOnly(MemorySource);

impl Source for MetadataOnly {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.0.columns(name)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("group validation cannot scan records");
    }
}

#[test]
fn every_nested_clause_rejects_ungrouped_captures_before_scanning() {
    let source = MetadataOnly(source());
    for sql in [
        "SELECT (SELECT O.Id) FROM Items O GROUP BY O.Bucket",
        "SELECT Count(*) FROM Items O GROUP BY O.Bucket HAVING EXISTS(SELECT 1 FROM Items I WHERE I.Id=O.Id)",
        "SELECT (SELECT I.Id FROM Items I ORDER BY O.Id) FROM Items O GROUP BY O.Bucket",
        "SELECT (SELECT Count(*) FROM Items I GROUP BY O.Id) FROM Items O GROUP BY O.Bucket",
        "SELECT (SELECT Count(*) FROM Items I HAVING O.Id=1) FROM Items O GROUP BY O.Bucket",
        "SELECT (SELECT Count(*) FROM Items I INNER JOIN Items J ON I.Id=J.Id AND J.Id=O.Id) FROM Items O GROUP BY O.Bucket",
        "SELECT (SELECT Sum(O.Id) FROM Items I) FROM Items O GROUP BY O.Bucket",
        "SELECT Count(*) FROM Items O GROUP BY O.Bucket HAVING O.Bucket IN(SELECT O.Bucket UNION ALL SELECT O.Id)",
        "SELECT (SELECT (SELECT O.Id)) FROM Items O GROUP BY O.Bucket",
        "SELECT (SELECT O.Bucket) FROM Items O GROUP BY O.Bucket Mod 2",
        "SELECT (SELECT 2 Mod O.Bucket) FROM Items O GROUP BY O.Bucket Mod 2",
        "SELECT Count(*) FROM Items O GROUP BY O.Bucket HAVING O.Id IN(SELECT 1)",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source).err(),
            Some(QueryError::Grouping),
            "{sql}"
        );
    }
}

#[test]
fn outer_aggregate_operands_still_evaluate_correlations_per_input_record() {
    assert_eq!(
        run("SELECT O.Bucket,Sum((SELECT O.Id)) FROM Items O GROUP BY O.Bucket ORDER BY O.Bucket")
            .unwrap(),
        [
            vec![Value::Int32(1), Value::Int64(3)],
            vec![Value::Int32(2), Value::Int64(7)],
            vec![Value::Int32(3), Value::Int64(5)]
        ]
    );
    assert_eq!(
        run("SELECT Count(*),(SELECT 1) FROM Items WHERE False").unwrap(),
        [vec![Value::Int64(0), Value::Int64(1)]]
    );
    assert_eq!(
        run("SELECT Count(*),(SELECT I.Id FROM Items I) FROM Items").err(),
        Some(QueryError::Cardinality)
    );
}

#[test]
fn isolated_derived_definitions_do_not_capture_the_enclosing_group_frame() {
    assert_eq!(run("SELECT O.Bucket,(SELECT Max(D.X) FROM (SELECT (SELECT I.Id) AS X FROM Items I) D) FROM Items O GROUP BY O.Bucket ORDER BY O.Bucket").unwrap(), [vec![Value::Int32(1),Value::Int32(5)],vec![Value::Int32(2),Value::Int32(5)],vec![Value::Int32(3),Value::Int32(5)]]);
}

#[test]
fn grouped_crosstab_having_and_null_keys_use_existing_subquery_semantics() {
    assert_eq!(run("TRANSFORM Count(*) SELECT O.Bucket FROM Items O GROUP BY O.Bucket HAVING EXISTS(SELECT I.Id FROM Items I WHERE I.Bucket=O.Bucket AND I.Id>3) PIVOT O.Id IN (1,2,3,4,5)").unwrap(), [
        vec![Value::Int32(2),Value::Null,Value::Null,Value::Int64(1),Value::Int64(1),Value::Null],
        vec![Value::Int32(3),Value::Null,Value::Null,Value::Null,Value::Null,Value::Int64(1)],
    ]);
    assert_eq!(run("SELECT Count(*),(SELECT Count(*) FROM (SELECT NULL AS Bucket) I WHERE I.Bucket=O.Bucket) FROM (SELECT NULL AS Bucket UNION ALL SELECT NULL) O GROUP BY O.Bucket").unwrap(), [vec![Value::Int64(2),Value::Int64(0)]]);
}

#[test]
fn grouped_subqueries_keep_cumulative_limits_and_all_cancellation_checkpoints() {
    let source = source();
    let parsed = parse_query(
        "SELECT O.Bucket,(SELECT Count(*) FROM Items I WHERE I.Bucket=O.Bucket) FROM Items O GROUP BY O.Bucket HAVING EXISTS(SELECT 1) ORDER BY O.Bucket",
        ParseLimits::default(),
    ).unwrap();
    for limits in [
        tablecraft_query::QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        tablecraft_query::QueryLimits {
            rows: 1,
            ..Default::default()
        },
        tablecraft_query::QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            tablecraft_query::execute_with_schema_discovery(
                parsed.query(),
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                },
            ),
            Err(QueryError::Limit(_))
        ));
    }
    let checks = std::cell::Cell::new(0);
    let count = || {
        checks.set(checks.get() + 1);
        false
    };
    tablecraft_query::execute_with_schema_discovery(
        parsed.query(),
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    let total = checks.get();
    assert!(total > 0);
    for stop in 1..=total {
        checks.set(0);
        let cancel = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        assert!(
            matches!(
                tablecraft_query::execute_with_schema_discovery(
                    parsed.query(),
                    &source,
                    &[],
                    ExecutionOptions {
                        cancelled: Some(&cancel),
                        ..Default::default()
                    }
                ),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}/{total}"
        );
    }
}
