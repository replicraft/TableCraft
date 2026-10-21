use super::*;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_sql::query::Query;

#[test]
fn parent_criteria_filter_cells_without_shrinking_dynamic_heading_domain() {
    let source = Collections::new();
    let sql = "TRANSFORM Count(*) SELECT Id FROM Items WHERE Tags.Value=7 GROUP BY Id ORDER BY Id PIVOT Id";
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(source.requests.borrow().is_empty());
    assert_eq!(source.ordinary.get(), 0);
    let result = plan
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.columns(), ["Id", "1", "2", "3"]);
    assert_eq!(
        result.rows(),
        [vec![
            Value::Int32(1),
            Value::Int64(1),
            Value::Null,
            Value::Null
        ]]
    );
    let result = run(&format!("{sql} IN (3,1,2)"), &source).unwrap();
    assert_eq!(result.columns(), ["Id", "3", "1", "2"]);
    assert_eq!(
        result.rows(),
        [vec![
            Value::Int32(1),
            Value::Null,
            Value::Int64(1),
            Value::Null
        ]]
    );
    let result = run(&sql.replace("Tags.Value=7", "Tags.Value=99"), &source).unwrap();
    assert_eq!(result.columns(), ["Id", "1", "2", "3"]);
    assert!(result.rows().is_empty());
}

#[test]
fn pivot_only_collections_are_displayed_and_independent_hidden_members_do_not_multiply_cells() {
    let source = Collections::new();
    let sql = "TRANSFORM Count(*) SELECT Id FROM Items WHERE Tags.Value=8 GROUP BY Id PIVOT Tags.Value IN (7,8)";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1), Value::Null, Value::Int64(2)]]
    );
    let sql = "TRANSFORM Count(*) SELECT 1 AS R FROM Items WHERE Other.Value IN (4,5) GROUP BY 1 PIVOT Tags.Value";
    let result = run(sql, &source).unwrap();
    assert_eq!(result.columns(), ["R", "7", "8"]);
    assert_eq!(
        result.rows(),
        [vec![Value::Int64(1), Value::Int64(1), Value::Int64(2)]]
    );
    let result = run(&format!("{sql} IN (8,7,99)"), &source).unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            Value::Int64(1),
            Value::Int64(2),
            Value::Int64(1),
            Value::Null
        ]]
    );
    let sql = "TRANSFORM Count(Tags.Value) SELECT Id FROM Items WHERE Other.Value=5 GROUP BY Id PIVOT Id IN (1,2)";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1), Value::Int64(3), Value::Null]]
    );
}

#[test]
fn joined_membership_keeps_absence_rules_and_complete_heading_discovery() {
    let source = Collections::new();
    let sql = "TRANSFORM Count(*) SELECT A.Id FROM Items AS A,Items AS B WHERE B.Other.Value=6 GROUP BY A.Id ORDER BY A.Id PIVOT B.Id";
    let result = run(sql, &source).unwrap();
    assert_eq!(result.columns(), ["Id", "1", "2", "3"]);
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(1), Value::Null, Value::Int64(1), Value::Null],
            vec![Value::Int32(2), Value::Null, Value::Int64(1), Value::Null],
            vec![Value::Int32(3), Value::Null, Value::Int64(1), Value::Null],
        ]
    );
    let sql = "TRANSFORM Count(*) SELECT A.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+1=B.Id WHERE B.Tags.Value IS NULL GROUP BY A.Id PIVOT A.Id";
    let result = run(sql, &source).unwrap();
    assert_eq!(result.columns(), ["Id", "1", "2", "3"]);
    assert_eq!(
        result.rows(),
        [vec![
            Value::Int32(2),
            Value::Null,
            Value::Int64(1),
            Value::Null
        ]]
    );
    let sql = "TRANSFORM Count(*) SELECT A.Tags.Value AS V FROM Items AS A,Items AS B WHERE B.Other.Value=5 GROUP BY A.Tags.Value ORDER BY V PIVOT A.Id IN (1,2,3)";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [
            vec![Value::Null, Value::Null, Value::Int64(1), Value::Int64(1)],
            vec![Value::Int32(7), Value::Int64(1), Value::Null, Value::Null],
            vec![Value::Int32(8), Value::Int64(2), Value::Null, Value::Null],
        ]
    );
}

#[test]
fn discovery_runs_once_before_membership_and_fixed_pivots_follow_membership() {
    struct Observed(Cell<usize>);
    impl Collation for Observed {
        fn compare(&self, left: &str, right: &str) -> Result<std::cmp::Ordering, EvalError> {
            assert_eq!((left, right), ("pivot", "pivot"));
            self.0.set(self.0.get() + 1);
            Ok(std::cmp::Ordering::Equal)
        }
    }
    let source = Collections::new();
    for from in [
        "Items AS A",
        "Items AS A INNER JOIN Items AS B ON A.Id=B.Id",
    ] {
        let sql = format!(
            "TRANSFORM Count(*) SELECT A.Id FROM {from} WHERE A.Tags.Value=7 GROUP BY A.Id PIVOT IIf('pivot'='pivot',A.Id,0)"
        );
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let observed = Observed(Cell::new(0));
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&observed),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(observed.0.get(), 3);
    }
    let sql =
        "TRANSFORM Count(*) SELECT Id FROM Items WHERE Tags.Value=7 GROUP BY Id PIVOT 10 \\ (Id-2)";
    assert_eq!(
        run(sql, &source).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert!(run(&format!("{sql} IN (-10,10)"), &source).is_ok());
    let mut source = Collections::new();
    source.ignore_errors = true;
    assert_eq!(
        run(sql, &source).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    let sql = "TRANSFORM Count(*) SELECT Id FROM Items WHERE False AND Tags.Value=7 GROUP BY Id PIVOT 10 \\ (Id-2)";
    assert_eq!(
        run(sql, &source).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    for sql in [
        "TRANSFORM Count(*) SELECT TOP 1 Id FROM Items WHERE True OR 1 \\ (Tags.Value-8)>0 GROUP BY Id PIVOT Id IN (1,2,3)",
        "TRANSFORM Sum(1 \\ (Id-3)) SELECT TOP 1 Id FROM Items WHERE NOT Other.Value=9 GROUP BY Id ORDER BY Id PIVOT Id IN (1,2,3)",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
}

struct Views {
    source: Collections,
    definition: Query,
}
impl Source for Views {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name
            .eq_ignore_ascii_case("Grid")
            .then_some(&self.definition))
    }
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.source.columns(name)
    }
    fn unavailable_columns(&self, name: &str) -> Result<&[SchemaName], QueryError> {
        self.source.unavailable_columns(name)
    }
    fn multivalue_columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.source.multivalue_columns(name)
    }
    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.source.scan(name, visitor)
    }
    fn scan_multivalues(
        &self,
        name: &str,
        indices: &[usize],
        collections: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.source
            .scan_multivalues(name, indices, collections, visitor)
    }
}

#[test]
fn fixed_named_crosstabs_rebind_membership_parameters_and_preserve_inner_errors() {
    let source = Views {
        source: Collections::new(),
        definition: parse_query("PARAMETERS Needle LONG; TRANSFORM Count(*) SELECT Id FROM Items WHERE (SELECT Tags.Value)=Needle GROUP BY Id PIVOT Id IN (1,2,3)", ParseLimits::default()).unwrap().into_query(),
    };
    let parsed = parse_query(
        "PARAMETERS Needle LONG; SELECT G.Id,G.[1] FROM Grid AS G ORDER BY G.Id",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(source.source.requests.borrow().is_empty());
    for (needle, expected) in [
        (7, vec![vec![Value::Int32(1), Value::Int64(1)]]),
        (99, vec![]),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &[("Needle", Value::Int32(needle))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
            expected
        );
    }
    let source = Views {
        source: Collections::new(),
        definition: parse_query("TRANSFORM Sum(1 \\ (Id-3)) SELECT Id FROM Items WHERE NOT Other.Value=9 GROUP BY Id PIVOT Id IN (1,2,3)", ParseLimits::default()).unwrap().into_query(),
    };
    assert_eq!(
        run("SELECT TOP 1 Id FROM Grid WHERE False", &source).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        failure_persistence: None,
        ..Default::default()
    })]
    #[test]
    fn generated_parent_grid_matches_independent_membership_and_pre_where_headings(
        lists in prop::collection::vec(
            prop::collection::vec(prop::option::of(0_i32..4), 0..5), 0..8,
        ),
        needle in 0_i32..4,
    ) {
        let mut source = Collections::new();
        source.rows = lists.iter().enumerate()
            .map(|(id, _)| vec![Value::Int32(99), Value::Int32(id as i32)])
            .collect();
        source.lists = lists.iter()
            .map(|list| vec![
                list.iter().map(|value| value.map_or(Value::Null, Value::Int32)).collect(),
                vec![],
            ])
            .collect();
        let sql = format!("TRANSFORM Count(*) SELECT Id FROM Items WHERE Tags.Value={needle} GROUP BY Id ORDER BY Id PIVOT Id");
        let result = run(&sql, &source).unwrap();
        let columns = std::iter::once("Id".to_owned())
            .chain((0..lists.len()).map(|id| id.to_string()))
            .collect::<Vec<_>>();
        let expected = lists.iter().enumerate()
            .filter(|(_, list)| list.contains(&Some(needle)))
            .map(|(id, _)| {
                std::iter::once(Value::Int32(id as i32))
                    .chain((0..lists.len()).map(|heading| {
                        if heading == id { Value::Int64(1) } else { Value::Null }
                    }))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        prop_assert_eq!(result.columns(), columns);
        prop_assert_eq!(result.rows(), expected);
    }
}

#[test]
fn parent_crosstabs_enforce_limits_schema_and_all_cancellation_checkpoints() {
    let source = Collections::new();
    let parsed = parse_query(
        "TRANSFORM Count(*) SELECT Id FROM Items WHERE Other.Value=5 GROUP BY Id PIVOT Tags.Value",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
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
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            ),
            Err(QueryError::Limit(_))
        ));
    }
    let calls = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == stop.get()
    };
    let execute = || {
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    execute().unwrap();
    for checkpoint in 1..=calls.get() {
        calls.set(0);
        stop.set(checkpoint);
        assert!(matches!(
            execute().err(),
            Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    source.changed.set(true);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::SchemaChanged)
    );
    let mut source = Collections::new();
    source.ignore_errors = true;
    source.lists[2][1].push(Value::Text("wrong carrier".into()));
    assert!(matches!(
        plan.execute(&source, &[], ExecutionOptions::default()),
        Err(QueryError::Schema(_))
    ));
}

#[test]
fn dynamic_heading_limits_still_apply_when_membership_rejects_every_parent() {
    let mut source = Collections::new();
    source.rows = (0..256)
        .map(|id| vec![Value::Int32(99), Value::Int32(id)])
        .collect();
    source.lists = (0..256)
        .map(|_| vec![vec![Value::Int32(7)], vec![]])
        .collect();
    let sql = "TRANSFORM Count(*) SELECT 1 AS R FROM Items WHERE Tags.Value=99 GROUP BY 1 PIVOT Id";
    assert_eq!(run(sql, &source).err(), Some(QueryError::StructureLimit));
}
