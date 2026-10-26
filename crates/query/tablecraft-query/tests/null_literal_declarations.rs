use std::cell::Cell;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, QueryLimits, Source, prepare_with_options,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default())
        .unwrap()
        .query()
        .clone()
}

fn run(sql: &str, source: &dyn Source, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let plan = prepare_with_options(
        &syntax(sql),
        source,
        PreparationOptions {
            declared_arithmetic: declared,
            ..Default::default()
        },
    )?;
    Ok(plan
        .execute(
            source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn literal_null_retains_variant_through_known_logical_results_and_arithmetic() {
    let source = MemorySource::default();
    for (expression, increment, expected) in [
        ("Null Or CByte(255)", "CByte(1)", Value::Int16(256)),
        ("CByte(255) Or Null", "CByte(1)", Value::Int16(256)),
        ("Not (CByte(0) And Null)", "CByte(1)", Value::Int16(256)),
        ("Null Or CInt(32767)", "CInt(1)", Value::Int32(32768)),
        ("CInt(32767) Or Null", "CInt(1)", Value::Int32(32768)),
        (
            "(Null And CInt(0))+CInt(32767)",
            "CInt(1)",
            Value::Int32(32768),
        ),
        (
            "Null Or CLng(2147483647)",
            "CLng(1)",
            Value::Double(Double::from_float(2147483648.0)),
        ),
    ] {
        for sql in [
            format!("SELECT ({expression})+{increment}"),
            format!("SELECT Q.N+{increment} FROM (SELECT {expression} AS N) Q"),
            format!("SELECT (SELECT Q.N+{increment}) FROM (SELECT {expression} AS N) Q"),
            format!("SELECT Q.N+{increment} FROM (SELECT {expression} AS N) Q GROUP BY Q.N"),
        ] {
            for declared in [false, true] {
                assert_eq!(
                    run(&sql, &source, declared).unwrap(),
                    [vec![expected.clone()]],
                    "declared {declared}: {sql}"
                );
            }
        }
    }
}

#[test]
fn empty_subqueries_and_matching_membership_keep_syntactic_null_provenance() {
    let source = MemorySource::default();
    for sql in [
        "SELECT CInt(-32768)\\(CInt(1)=ALL(SELECT NULL WHERE False))",
        "SELECT CInt(-32768)\\(CInt(1) IN (CInt(1),NULL))",
        "SELECT CInt(-32768)\\(CInt(1) IN (NULL,CInt(1)))",
    ] {
        assert_eq!(
            run(sql, &source, true).unwrap(),
            [vec![Value::Int32(32768)]],
            "{sql}"
        );
    }
    assert_eq!(
        run(
            "SELECT CInt(-32768)\\(CInt(1)=ALL(SELECT CInt(0) WHERE False))",
            &source,
            true,
        ),
        Err(EvalError::Overflow.into())
    );
}

#[test]
fn runtime_null_fields_parameters_and_unknown_aggregates_are_not_reclassified() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int16, true).unwrap()],
            vec![vec![Value::Null]],
        )
        .unwrap(),
    ])
    .unwrap();
    assert_eq!(
        run("SELECT (N Or CInt(32767))+CInt(1) FROM T", &source, true),
        Err(EvalError::Overflow.into())
    );

    let plan = prepare_with_options(
        &syntax("PARAMETERS N SHORT; SELECT (N Or CInt(32767))+CInt(1)"),
        &source,
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        plan.execute(
            &source,
            &[("N", Value::Null)],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        ),
        Err(QueryError::Evaluation(EvalError::Overflow))
    ));

    let variant_parameter = prepare_with_options(
        &syntax("PARAMETERS N VALUE; SELECT (N Or CInt(32767))+CInt(1)"),
        &source,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        variant_parameter
            .execute(
                &source,
                &[("N", Value::Null)],
                ExecutionOptions {
                    declared_integral: true,
                    ..Default::default()
                },
            )
            .unwrap()
            .rows(),
        [vec![Value::Int32(32768)]]
    );

    for sql in [
        "SELECT First(NULL Or CInt(32767))+CInt(1) FROM T",
        "SELECT Q.N+CInt(1) FROM (SELECT NULL Or CInt(32767) AS N UNION ALL SELECT CInt(0) WHERE False) Q",
    ] {
        assert_eq!(
            run(sql, &source, true),
            Err(QueryError::ArithmeticDeclaration)
        );
    }
}

struct Named {
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

    fn columns(&self, _name: &str) -> Result<&[Column], QueryError> {
        Err(QueryError::UnknownName)
    }

    fn scan(
        &self,
        _name: &str,
        _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        Err(QueryError::Source)
    }
}

#[test]
fn named_and_fixed_crosstab_outputs_keep_literal_categories_without_scans() {
    let source = Named {
        queries: [
            ("NullSource", "SELECT NULL AS N"),
            ("KnownResult", "SELECT N Or CInt(32767) AS N FROM NullSource"),
            (
                "PivotResult",
                "TRANSFORM Count(*) SELECT NULL Or CInt(32767) AS G GROUP BY NULL Or CInt(32767) PIVOT 1 IN (1)",
            ),
            (
                "LiteralHeading",
                "TRANSFORM Count(*) SELECT CInt(1) AS G GROUP BY CInt(1) PIVOT ((NULL Or CInt(32767))+CInt(1)) IN ((NULL Or CInt(32767))+CInt(1))",
            ),
        ]
        .into_iter()
        .map(|(name, sql)| (name.into(), syntax(sql)))
        .collect(),
        scans: Cell::new(0),
    };
    for (sql, expected) in [
        ("SELECT N+CInt(1) FROM KnownResult", Value::Int32(32768)),
        ("SELECT G+CInt(1) FROM PivotResult", Value::Int32(32768)),
        ("SELECT P.[32768] FROM LiteralHeading P", Value::Int64(1)),
    ] {
        assert_eq!(run(sql, &source, true).unwrap(), [vec![expected]], "{sql}");
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn concatenated_null_literals_keep_variant_string_dispatch_through_outer_operations() {
    let source = MemorySource::default();
    for sql in [
        "SELECT (NULL &'2')+CInt(3)",
        "SELECT CInt(3)+(NULL &'2')",
        "SELECT Q.N+CInt(3) FROM (SELECT NULL &'2' AS N) Q",
        "SELECT ((NULL &'2')+'2')+CInt(3)-CInt(20)",
    ] {
        for declared in [false, true] {
            let plan = prepare_with_options(
                &syntax(sql),
                &source,
                PreparationOptions {
                    declared_arithmetic: declared,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral: declared,
                        number_format: Some(NumberFormat::Invariant),
                        ..Default::default()
                    },
                )
                .unwrap()
                .rows(),
                [vec![Value::Double(Double::from_float(5.0))]],
                "declared {declared}: {sql}"
            );
        }
    }
}

#[test]
fn progressive_results_and_reused_plans_share_the_literal_rule() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("ID".into(), ScalarType::Int32, false).unwrap()],
            vec![vec![Value::Int32(1)]],
        )
        .unwrap(),
    ])
    .unwrap();
    let plan = prepare_with_options(
        &syntax("SELECT (NULL Or CInt(32767))+CInt(1) AS N FROM T"),
        &source,
        Default::default(),
    )
    .unwrap();
    let options = ExecutionOptions {
        declared_integral: true,
        ..Default::default()
    };
    let mut rows = Vec::new();
    plan.execute_progressive(&source, &[], options, &mut |row| {
        rows.push(row.to_vec());
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(rows, [vec![Value::Int32(32768)]]);

    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| true),
                ..options
            },
        ),
        Err(QueryError::Cancelled)
    ));
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    steps: 1,
                    ..QueryLimits::default()
                },
                ..options
            },
        ),
        Err(QueryError::Limit(_))
    ));
    assert_eq!(
        plan.execute(&source, &[], options).unwrap().rows(),
        [vec![Value::Int32(32768)]]
    );
}

#[test]
fn known_scalar_casts_longlong_bounds_and_eager_failures_remain_explicit() {
    let source = MemorySource::default();
    assert_eq!(
        run("SELECT CInt(NULL Or CInt(32767))+CInt(1)", &source, true),
        Err(EvalError::Overflow.into())
    );
    let plan = prepare_with_options(
        &syntax("PARAMETERS N BIGINT; SELECT (NULL Or N)+CLngLng(1)"),
        &source,
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        plan.execute(
            &source,
            &[("N", Value::Int64(i64::MAX))],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        ),
        Err(QueryError::Evaluation(EvalError::Overflow))
    ));
    for sql in [
        "SELECT (NULL And CInt(0))+CInt(1/0)",
        "SELECT IIf(True,(NULL Or CInt(32767))+CInt(1),1/0)",
    ] {
        assert_eq!(
            run(sql, &source, true),
            Err(EvalError::DivisionByZero.into())
        );
    }
    assert_eq!(
        run(
            "SELECT NULL+CInt(1),Not NULL,NULL Xor CInt(0)",
            &source,
            true
        )
        .unwrap(),
        [vec![Value::Null, Value::Null, Value::Null]]
    );
}
