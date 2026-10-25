//! Flat FROM roots retain bounded ordinary binding and execution at 32 tables.
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct Tables {
    inner: MemorySource,
    metadata: Cell<usize>,
    scans: Cell<usize>,
}

impl Tables {
    fn new(empty: Option<usize>) -> Self {
        let tables = (1..=33)
            .map(|number| {
                let rows = if empty == Some(number) {
                    Vec::new()
                } else {
                    vec![vec![Value::Int32(number as i32)]]
                };
                MemoryTable::new(
                    format!("T{number}"),
                    vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
                    rows,
                )
                .unwrap()
            })
            .collect();
        Self {
            inner: MemorySource::new(tables).unwrap(),
            metadata: Cell::new(0),
            scans: Cell::new(0),
        }
    }
}

impl Source for Tables {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.metadata.set(self.metadata.get() + 1);
        self.inner.columns(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.inner.scan(table, visitor)
    }
}

fn roots(count: usize) -> String {
    (1..=count)
        .map(|number| format!("T{number}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn projection() -> String {
    (1..=32)
        .map(|number| format!("T{number}.Id AS V{number}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn thirty_two_roots_bind_without_scanning_and_preserve_every_projected_field() {
    let wildcards = (1..=32)
        .map(|number| format!("T{number}.*"))
        .collect::<Vec<_>>()
        .join(",");
    for projection in [projection(), wildcards] {
        let source = Tables::new(None);
        let sql = format!("SELECT {projection} FROM {}", roots(32));
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(plan.output_types().unwrap().len(), 32);

        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert_eq!(
            result.rows(),
            [(1..=32).map(Value::Int32).collect::<Vec<_>>()]
        );
        assert_eq!(source.scans.get(), 32);
    }
}

#[test]
fn thirty_two_roots_keep_filters_and_empty_table_cardinality() {
    for empty in [None, Some(17)] {
        let source = Tables::new(empty);
        let sql = format!(
            "SELECT T1.Id AS StartId,T32.Id AS EndId FROM {} WHERE T32.Id=T1.Id+31",
            roots(32)
        );
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        if empty.is_some() {
            assert!(result.rows().is_empty());
        } else {
            assert_eq!(result.rows(), [vec![Value::Int32(1), Value::Int32(32)]]);
        }
    }
}

#[test]
fn crosstab_execution_accepts_thirty_two_flat_sources() {
    let source = Tables::new(None);
    let sql = format!(
        "TRANSFORM Count(*) SELECT T1.Id FROM {} GROUP BY T1.Id PIVOT T32.Id IN (32)",
        roots(32)
    );
    let syntax = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Int32(1), Value::Int64(1)]]
    );
    assert_eq!(source.scans.get(), 32);
}

#[test]
fn thirty_three_flat_sources_refuse_before_any_source_metadata_or_records() {
    for sql in [
        format!("SELECT T1.Id FROM {}", roots(33)),
        format!(
            "TRANSFORM Count(*) SELECT T1.Id FROM {} GROUP BY T1.Id PIVOT T33.Id IN (33)",
            roots(33)
        ),
    ] {
        let source = Tables::new(None);
        let syntax = parse_query(&sql, Default::default()).unwrap();
        assert!(matches!(
            prepare(syntax.query(), &source),
            Err(QueryError::StructureLimit)
        ));
        assert_eq!(source.metadata.get(), 0);
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn wider_from_lists_keep_cumulative_execution_limits_and_cancellation() {
    let source = Tables::new(None);
    let sql = format!("SELECT T1.Id FROM {}", roots(32));
    let syntax = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for limits in [
        QueryLimits {
            bytes: 1,
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
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() > 100
    };
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            }
        ),
        Err(QueryError::Cancelled)
    ));
}
