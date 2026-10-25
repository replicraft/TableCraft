//! Projection aliases compose within their written SELECT, never another scope.
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

struct Host {
    data: MemorySource,
    named: Vec<(String, Query)>,
    scans: Cell<usize>,
}

impl Host {
    fn new(definitions: &[(&str, &str)]) -> Self {
        let columns = vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()];
        let rows = [1, 3, 5].map(|value| vec![Value::Int32(value)]).to_vec();
        let table = MemoryTable::new("T".into(), columns, rows).unwrap();
        Self {
            data: MemorySource::new(vec![table]).unwrap(),
            named: definitions
                .iter()
                .map(|(name, sql)| {
                    let syntax = parse_query(sql, Default::default()).unwrap();
                    ((*name).into(), syntax.into_query())
                })
                .collect(),
            scans: Cell::new(0),
        }
    }
}

impl Source for Host {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.data.columns(table)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .named
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data.scan(table, visitor)
    }
}

#[test]
fn correlated_aggregate_alias_drives_the_later_interval_projection() {
    let source = Host::new(&[]);
    let syntax = parse_query(
        "SELECT N,(SELECT Max(Old.N) FROM T AS Old WHERE Old.N<Current.N) AS Prior,
         N-Prior AS Distance FROM T AS Current ORDER BY N",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = plan.execute(&source, &[], Default::default()).unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(1), Value::Null, Value::Null],
            vec![Value::Int32(3), Value::Int32(1), Value::Int32(2)],
            vec![Value::Int32(5), Value::Int32(3), Value::Int32(2)],
        ]
    );
}

#[test]
fn named_derived_and_union_wrappers_preserve_the_alias_owner() {
    let source = Host::new(&[("Computed", "SELECT N+1 AS Seed,Seed*2 AS Twice FROM T")]);
    let syntax = parse_query(
        "SELECT Q.Seed,Q.Twice FROM (SELECT * FROM Computed
         UNION ALL SELECT * FROM Computed) AS Q ORDER BY Q.Seed",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    for declared in [false, true] {
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: declared,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [
                vec![Value::Int64(2), Value::Int64(4)],
                vec![Value::Int64(2), Value::Int64(4)],
                vec![Value::Int64(4), Value::Int64(8)],
                vec![Value::Int64(4), Value::Int64(8)],
                vec![Value::Int64(6), Value::Int64(12)],
                vec![Value::Int64(6), Value::Int64(12)],
            ]
        );
    }
}

#[test]
fn nested_union_sort_cannot_capture_an_outer_projection_alias() {
    let source = Host::new(&[]);
    for sql in [
        "SELECT 1 AS Secret,(SELECT 2 AS X UNION ALL SELECT 3 ORDER BY Secret)",
        "SELECT 1 AS Secret,(SELECT 2 AS X UNION ALL SELECT 3 ORDER BY Secret+1)",
        "SELECT 1 AS Secret,(SELECT Secret)",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        assert!(
            matches!(
                prepare(syntax.query(), &source),
                Err(QueryError::UnknownName)
            ),
            "{sql}"
        );
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn unsupported_alias_dependencies_fail_before_any_records() {
    let source = Host::new(&[]);
    for sql in [
        "SELECT Later AS Earlier,1 AS Later FROM T",
        "SELECT SelfValue+1 AS SelfValue FROM T",
        "SELECT 1 AS Repeated,2 AS Repeated,Repeated FROM T",
        "SELECT Rnd() AS Draw,Draw FROM T",
        "SELECT Now() AS Moment,Moment FROM T",
        "SELECT Eval('1') AS DynamicValue,DynamicValue FROM T",
        "SELECT DCount('*','T') AS DomainValue,DomainValue FROM T",
        "SELECT Count(*) AS Total,Total+1 FROM T",
        "SELECT N AS KeyValue,KeyValue FROM T GROUP BY N",
        "SELECT N AS Seed,Sum(Seed) FROM T",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        assert!(prepare(syntax.query(), &source).is_err(), "{sql}");
        assert_eq!(source.scans.get(), 0);
    }
}
