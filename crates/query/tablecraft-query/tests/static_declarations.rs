use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

fn run(sql: &str, source: &dyn Source) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(
            source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn subquery_categories_follow_bound_expressions_not_result_populations() {
    let source = MemorySource::default();
    for (expression, variant) in [
        ("CInt(1)", false),
        ("CVar(CInt(1))", true),
        ("Abs(CInt(1))", true),
        ("Sqr(CVar(1))", false),
        ("SLN(1,0,1)", false),
        ("CInt(CVar(1))", false),
        ("CVar(CInt(1))+CInt(0)", true),
        ("CInt(1)+CInt(0)", false),
    ] {
        for query in [
            format!("SELECT {expression} AS N"),
            format!("SELECT Q.N FROM (SELECT {expression} AS N) AS Q"),
            format!("SELECT A.N FROM (SELECT {expression} AS N) AS A,(SELECT 2 AS K) AS B"),
        ] {
            for predicate in [
                format!("CInt(1) IN ({query})"),
                format!("CInt(1)=ALL({query} WHERE False)"),
                format!("CInt(1) NOT IN ({query} WHERE False)"),
            ] {
                let sql = format!("SELECT CInt(-32768)\\({predicate})");
                if variant {
                    assert_eq!(
                        run(&sql, &source).unwrap(),
                        vec![vec![Value::Int32(32768)]],
                        "{sql}"
                    );
                } else {
                    assert_eq!(run(&sql, &source), Err(EvalError::Overflow.into()), "{sql}");
                }
            }
        }
    }
}

#[test]
fn empty_scalar_subqueries_and_correlated_outputs_keep_static_categories() {
    let source = MemorySource::default();
    for (expression, variant) in [("CInt(1)", false), ("CVar(CInt(1))", true)] {
        for sql in [
            format!(
                "SELECT CInt(-32768)\\((SELECT {expression} WHERE False)=ALL(SELECT CInt(0) WHERE False))"
            ),
            format!(
                "SELECT CInt(-32768)\\(CInt(1)=ALL(SELECT Q.N WHERE False)) FROM (SELECT {expression} AS N) AS Q"
            ),
        ] {
            if variant {
                assert_eq!(
                    run(&sql, &source).unwrap(),
                    vec![vec![Value::Int32(32768)]],
                    "{sql}"
                );
            } else {
                assert_eq!(run(&sql, &source), Err(EvalError::Overflow.into()), "{sql}");
            }
        }
    }
}

struct Named {
    table: MemorySource,
    queries: Vec<(String, Query)>,
    scans: Cell<usize>,
}

impl Source for Named {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .queries
            .iter()
            .find_map(|(label, query)| (label == name).then_some(query)))
    }
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.table.columns(name)
    }
    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.table.scan(name, visitor)
    }
}

#[test]
fn empty_named_sources_and_crosstab_groups_bind_without_reading_records() {
    let source = Named {
        table: MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("N".into(), ScalarType::Int16, false).unwrap()],
                vec![],
            ).unwrap(),
        ]).unwrap(),
        queries: [
            ("VariantView", "SELECT CVar(N) AS V FROM T"),
            ("ScalarView", "SELECT N AS V FROM T"),
            ("PivotView", "TRANSFORM Count(*) SELECT CVar(CInt(1)) AS G FROM T GROUP BY CVar(CInt(1)) PIVOT N IN (1)"),
        ]
        .into_iter()
        .map(|(name, sql)| {
            let parsed = parse_query(sql, Default::default()).unwrap();
            (name.into(), parsed.query().clone())
        })
        .collect(),
        scans: Cell::new(0),
    };
    for (name, column, variant) in [
        ("VariantView", "V", true),
        ("ScalarView", "V", false),
        ("PivotView", "G", true),
    ] {
        source.scans.set(0);
        let sql = format!("SELECT CInt(-32768)\\(CInt(1)=ALL(SELECT {column} FROM {name}))");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        );
        if variant {
            assert_eq!(
                result.unwrap().rows(),
                &[vec![Value::Int32(32768)]],
                "{sql}"
            );
        } else {
            assert_eq!(result.err(), Some(EvalError::Overflow.into()), "{sql}");
        }
        assert!(source.scans.get() > 0);
    }
}

#[test]
fn union_and_aggregate_outputs_remain_unknown_including_empty_results() {
    let source = MemorySource::default();
    for query in [
        "SELECT CInt(1) AS N UNION ALL SELECT CVar(CInt(1)) WHERE False",
        "SELECT Count(*) FROM (SELECT 1 AS N WHERE False) AS Q GROUP BY N",
    ] {
        let sql = format!("SELECT CInt(-32768)\\(CInt(1)=ALL({query}))");
        assert_eq!(
            run(&sql, &source),
            Err(QueryError::ArithmeticDeclaration),
            "{sql}"
        );
    }
}
