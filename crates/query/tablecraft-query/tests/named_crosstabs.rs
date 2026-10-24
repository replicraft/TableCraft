use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
};
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, QueryResult, Source, prepare,
    source::{MemorySource, MemoryTable, ProjectedRow, ScanConstraint},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

const FIXED: &str = "TRANSFORM Sum(Amount) SELECT Region,Sum(Amount) AS Total FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter IN (2,1,4)";
struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
        Ok(a.cmp(b))
    }
}
struct Views {
    data: MemorySource,
    queries: Vec<(String, Query)>,
    scans: Cell<usize>,
    projections: RefCell<Vec<Vec<usize>>>,
}
impl Views {
    fn new(definitions: &[(&str, &str)]) -> Self {
        Self {
            data: MemorySource::new(vec![
                MemoryTable::new(
                    "Sales".into(),
                    vec![
                        Column::new("Region".into(), ScalarType::Int32, false).unwrap(),
                        Column::new("Quarter".into(), ScalarType::Int32, true).unwrap(),
                        Column::new("Amount".into(), ScalarType::Int32, true).unwrap(),
                        Column::new("Unused".into(), ScalarType::Text, false).unwrap(),
                    ],
                    [
                        (1, Some(2), Some(10)),
                        (1, Some(1), Some(3)),
                        (1, Some(2), Some(7)),
                        (2, Some(3), Some(5)),
                        (2, Some(1), None),
                        (3, None, Some(8)),
                    ]
                    .into_iter()
                    .map(|(r, q, a)| {
                        vec![
                            Value::Int32(r),
                            q.map_or(Value::Null, Value::Int32),
                            a.map_or(Value::Null, Value::Int32),
                            Value::Text("not needed".into()),
                        ]
                    })
                    .collect(),
                )
                .unwrap(),
            ])
            .unwrap(),
            queries: definitions
                .iter()
                .map(|(name, sql)| {
                    (
                        (*name).into(),
                        parse_query(sql, Default::default()).unwrap().into_query(),
                    )
                })
                .collect(),
            scans: Cell::new(0),
            projections: RefCell::new(Vec::new()),
        }
    }
    fn run(&self, sql: &str, parameters: &[(&str, Value)]) -> Result<QueryResult, QueryError> {
        let query = parse_query(sql, Default::default()).unwrap();
        prepare(query.query(), self)?.execute(
            self,
            parameters,
            ExecutionOptions {
                collation: Some(&Ordinal),
                ..Default::default()
            },
        )
    }
}
impl Source for Views {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .queries
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.data.columns(name)
    }
    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data.scan(name, visitor)
    }
    fn scan_projected(
        &self,
        name: &str,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(constraint.is_none());
        self.scans.set(self.scans.get() + 1);
        self.projections.borrow_mut().push(indices.to_vec());
        self.data.scan_projected(name, indices, None, visitor)
    }
}

#[test]
fn fixed_named_schemas_bind_without_scanning_and_keep_pivot_only_fields_after_repeated_pruning() {
    let source = Views::new(&[("Piv", FIXED)]);
    let query = parse_query(
        "SELECT p.Region,p.[2],p.[1],p.[4] FROM Piv AS p ORDER BY p.Region",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = plan.execute(&source, &[], Default::default()).unwrap();
    assert_eq!(result.columns(), ["Region", "2", "1", "4"]);
    assert_eq!(
        result.rows(),
        &[
            vec![
                Value::Int32(1),
                Value::Int64(17),
                Value::Int64(3),
                Value::Null
            ],
            vec![Value::Int32(2), Value::Null, Value::Null, Value::Null],
            vec![Value::Int32(3), Value::Null, Value::Null, Value::Null],
        ]
    );
    assert_eq!(source.projections.borrow().as_slice(), [vec![0, 1, 2]]);
    assert_eq!(
        source
            .run("SELECT Region FROM Piv WHERE [1]>1 ORDER BY Region", &[])
            .unwrap()
            .rows(),
        &[vec![Value::Int32(1)]]
    );
}

#[test]
fn named_crosstabs_compose_with_joins_unions_derived_reads_subqueries_and_another_crosstab() {
    let source = Views::new(&[
        ("Piv", FIXED),
        (
            "Again",
            "TRANSFORM Sum([1]) SELECT 0 AS Bucket FROM Piv GROUP BY 0 PIVOT Region IN (1,2)",
        ),
    ]);
    for sql in [
        "SELECT a.Region,a.[1] FROM Piv AS a INNER JOIN Piv AS b ON a.Region=b.Region WHERE b.Region=1",
        "SELECT d.Region,d.[1] FROM (SELECT * FROM Piv) AS d WHERE d.Region=1",
        "SELECT Region,[1] FROM Piv WHERE Region=1 UNION SELECT Region,[1] FROM Piv WHERE Region=1",
        "SELECT (SELECT [1] FROM Piv WHERE Region=1) AS Cell,1 AS Region",
    ] {
        let result = source.run(sql, &[]).unwrap();
        if sql.starts_with("SELECT (SELECT") {
            assert_eq!(result.rows(), &[vec![Value::Int64(3), Value::Int64(1)]]);
        } else {
            assert_eq!(result.rows(), &[vec![Value::Int32(1), Value::Int64(3)]]);
        }
    }
    assert_eq!(source.run("SELECT Region FROM Piv WHERE EXISTS (SELECT Region FROM Piv AS q WHERE q.Region=1) ORDER BY Region",&[]).unwrap().rows(),&[vec![Value::Int32(1)],vec![Value::Int32(2)],vec![Value::Int32(3)]]);
    assert_eq!(
        source.run("SELECT * FROM Again", &[]).unwrap().rows(),
        &[vec![Value::Int64(0), Value::Int64(3), Value::Null]]
    );
}

#[test]
fn literal_text_heading_punctuation_has_qualified_column_syntax_and_explicit_collation() {
    let source = Views::new(&[(
        "Piv",
        "TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region ORDER BY Region PIVOT IIf(Quarter=1,'Sales.Q1','Sales!Q2') IN ('Sales.Q1','Sales!Q2')",
    )]);
    let sql = "SELECT p.Region,p.[Sales.Q1],p.[Sales!Q2] FROM Piv AS p ORDER BY p.Region";
    let query = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert!(plan.execute(&source, &[], Default::default()).is_err());
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        source.run(sql, &[]).unwrap().rows(),
        &[
            vec![Value::Int32(1), Value::Int64(1), Value::Int64(2)],
            vec![Value::Int32(2), Value::Int64(1), Value::Int64(1)],
            vec![Value::Int32(3), Value::Null, Value::Int64(1)],
        ]
    );
}

#[test]
fn fixed_schema_limits_collisions_and_dependent_headings_fail_before_scans() {
    for (headings, error) in [
        ("", QueryError::Unsupported),
        (" IN (NULL)", QueryError::Unsupported),
        (" IN (TRUE,TRUE)", QueryError::AmbiguousName),
        (" IN ('')", QueryError::Unsupported),
        (" IN ('Region')", QueryError::AmbiguousName),
        (" IN (1,1)", QueryError::AmbiguousName),
        (" IN ('a','A')", QueryError::AmbiguousName),
        (" IN (P)", QueryError::Unsupported),
    ] {
        let text = format!(
            "PARAMETERS P LONG; TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter{headings}"
        );
        let source = Views::new(&[("Piv", &text)]);
        let query = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source).err(),
            Some(error),
            "{headings}"
        );
        assert_eq!(source.scans.get(), 0);
    }
    for count in [255, 256] {
        let headings = (0..count)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let text = format!(
            "TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter IN ({headings})"
        );
        let source = Views::new(&[("Piv", &text)]);
        let query = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
        let result = prepare(query.query(), &source);
        if count == 255 {
            assert_eq!(
                result
                    .unwrap()
                    .execute(&source, &[], Default::default())
                    .unwrap()
                    .columns()
                    .len(),
                256
            );
        } else {
            assert_eq!(result.err(), Some(QueryError::StructureLimit));
        }
    }
    let source = Views::new(&[(
        "Piv",
        "TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter IN (-1,+1,-9223372036854775808,0)",
    )]);
    assert_eq!(
        source.run("SELECT * FROM Piv", &[]).unwrap().columns(),
        ["Region", "-1", "1", "-9223372036854775808", "0"]
    );
}

#[test]
fn fixed_named_dependencies_keep_cycles_parameters_and_scope_isolation() {
    let source = Views::new(&[
        (
            "Piv",
            "PARAMETERS [Floor.] LONG; TRANSFORM Sum(Amount) SELECT Region FROM Sales WHERE Region>=[Floor.] GROUP BY Region PIVOT Quarter IN (1,2)",
        ),
        (
            "Unbound",
            "TRANSFORM Count(*) SELECT Region FROM Sales WHERE Region>=[Floor.] GROUP BY Region PIVOT Quarter IN (1,2)",
        ),
    ]);
    assert_eq!(
        source
            .run(
                "SELECT Region FROM Piv ORDER BY Region",
                &[("Floor.", Value::Int32(2))]
            )
            .unwrap()
            .rows(),
        &[vec![Value::Int32(2)], vec![Value::Int32(3)]]
    );
    assert_eq!(
        source.run("SELECT * FROM Piv", &[]).err(),
        Some(QueryError::Parameter)
    );
    assert_eq!(
        source
            .run(
                "PARAMETERS [Floor.] LONG; SELECT * FROM Unbound",
                &[("Floor.", Value::Int32(2))]
            )
            .err(),
        Some(QueryError::UnknownName)
    );
    let source = Views::new(&[
        (
            "A",
            "TRANSFORM Count(*) SELECT Region FROM B GROUP BY Region PIVOT Region IN (1)",
        ),
        ("B", "SELECT Region FROM A"),
    ]);
    assert_eq!(
        source.run("SELECT * FROM A", &[]).err(),
        Some(QueryError::DependencyCycle)
    );
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn outer_projection_and_filters_never_hide_inner_cells_and_limits_cancel_without_partial_results() {
    let source = Views::new(&[(
        "Piv",
        "TRANSFORM Sum(1/(Quarter-3)) SELECT Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter IN (1,2,3)",
    )]);
    for sql in [
        "SELECT Region FROM Piv",
        "SELECT Region FROM Piv WHERE Region=1",
        "SELECT TOP 1 Region FROM Piv ORDER BY Region",
    ] {
        assert!(matches!(
            source.run(sql, &[]),
            Err(QueryError::Evaluation(_))
        ));
    }
    let source = Views::new(&[("Piv", FIXED)]);
    let query = parse_query("SELECT Region FROM Piv ORDER BY Region", Default::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for limits in [
        QueryLimits {
            bytes: 128,
            ..Default::default()
        },
        QueryLimits {
            rows: 1,
            ..Default::default()
        },
        QueryLimits {
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
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&|| {
                calls.set(calls.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();
    for stop in 1..=calls.get() {
        let checks = Cell::new(0);
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| {
                        checks.set(checks.get() + 1);
                        checks.get() == stop
                    }),
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}

#[test]
fn named_fixed_top_retains_heading_width_ties_and_errors_in_excluded_groups() {
    for (top, order, regions) in [
        ("1", "Region", vec![1]),
        ("50 PERCENT", "Region", vec![1, 2]),
        ("1", "Region Mod 2 DESC", vec![1, 3]),
    ] {
        let text = format!(
            "TRANSFORM Sum(Amount) SELECT TOP {top} Region FROM Sales GROUP BY Region ORDER BY {order} PIVOT Quarter IN (2,1,4)"
        );
        let source = Views::new(&[("Piv", &text)]);
        let result = source.run("SELECT * FROM Piv", &[]).unwrap();
        assert_eq!(result.columns(), ["Region", "2", "1", "4"]);
        assert_eq!(
            result
                .rows()
                .iter()
                .map(|row| row[0].clone())
                .collect::<Vec<_>>(),
            regions.into_iter().map(Value::Int32).collect::<Vec<_>>()
        );
    }
    let source = Views::new(&[(
        "Piv",
        "TRANSFORM Sum(1/(Region-2)) SELECT TOP 1 Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter IN (1,2,3)",
    )]);
    assert!(matches!(
        source.run("SELECT Region FROM Piv", &[]),
        Err(QueryError::Evaluation(_))
    ));
}

#[path = "named_crosstabs/computed.rs"]
mod computed;

#[path = "named_crosstabs/conditional.rs"]
mod conditional;

#[path = "named_crosstabs/discovery.rs"]
mod discovery;
