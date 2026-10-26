//! Explicit operator-specific division, with metadata independent of row samples.
use std::cell::Cell;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, OutputType, PreparationOptions, QueryError, Source, prepare_with_options,
    source::MemorySource,
};
use tablecraft_schema::ScalarType;
use tablecraft_sql::{parse_query, query::Query};

fn scalar_type(output: OutputType) -> Option<ScalarType> {
    match output {
        OutputType::Scalar { data_type, .. } => Some(data_type.scalar_type()),
        OutputType::Null | OutputType::Dynamic => None,
        OutputType::Multivalue { .. } => {
            panic!("scalar result unexpectedly has collection metadata")
        }
    }
}

#[test]
fn one_prepared_plan_selects_runtime_profile_independently_of_literal_preparation() {
    let source = MemorySource::default();
    for prepared_declared in [false, true] {
        for peer in ["CDbl(2)", "CCur(2)", "CDate(2)", "'2'", "LeftB('2',2)"] {
            for expression in [format!("CDec(3)/{peer}"), format!("{peer}/CDec(3)")] {
                let sql = format!("SELECT {expression} AS V, VarType({expression}) AS K");
                let parsed = parse_query(&sql, Default::default()).unwrap();
                let plan = prepare_with_options(
                    parsed.query(),
                    &source,
                    PreparationOptions {
                        declared_arithmetic: prepared_declared,
                        ..Default::default()
                    },
                )
                .unwrap();
                assert_eq!(plan.output_types().unwrap()[0], OutputType::Dynamic);
                for declared in [false, true] {
                    let result = plan
                        .execute(
                            &source,
                            &[],
                            ExecutionOptions {
                                declared_integral: declared,
                                number_format: Some(NumberFormat::Invariant),
                                ..Default::default()
                            },
                        )
                        .unwrap();
                    assert_eq!(
                        scalar_type(result.output_types()[0]),
                        Some(if declared {
                            ScalarType::Double
                        } else {
                            ScalarType::Decimal
                        })
                    );
                    assert_eq!(
                        result.rows()[0][1],
                        Value::Int16(if declared { 5 } else { 14 })
                    );
                }
            }
        }
    }
}

#[test]
fn union_promotes_static_profiles_before_nested_selectors_and_aggregates() {
    let source = MemorySource::default();
    for outer in [
        "V",
        "IIf(True,V,CDec(0))",
        "Choose(1,V,CDec(0))",
        "Switch(True,V,False,CDec(0))",
        "Sum(V)",
        "Min(V)",
        "Max(V)",
    ] {
        let sql = format!(
            "SELECT {outer} AS X FROM (SELECT CDec(3)/CDbl(2) AS V UNION ALL SELECT CLng(2)) AS Q"
        );
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare_with_options(parsed.query(), &source, Default::default()).unwrap();
        assert_eq!(plan.output_types().unwrap()[0], OutputType::Dynamic);
        for declared in [false, true] {
            let result = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral: declared,
                        number_format: Some(NumberFormat::Invariant),
                        ..Default::default()
                    },
                )
                .unwrap();
            for row in result.rows() {
                assert!(
                    if declared {
                        matches!(row[0], Value::Double(_))
                    } else {
                        matches!(row[0], Value::Decimal(_))
                    },
                    "{outer}"
                );
            }
            assert_eq!(
                scalar_type(result.output_types()[0]),
                Some(if declared {
                    ScalarType::Double
                } else {
                    ScalarType::Decimal
                })
            );
        }
    }
}

struct Named {
    definition: Query,
    scans: Cell<usize>,
    rows_allowed: Cell<bool>,
    empty: MemorySource,
}

impl Source for Named {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "Quotients").then_some(&self.definition))
    }

    fn columns(&self, table: &str) -> Result<&[tablecraft_schema::Column], QueryError> {
        self.empty.columns(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(self.rows_allowed.get());
        self.scans.set(self.scans.get() + 1);
        self.empty.scan(table, visitor)
    }
}

#[test]
fn named_metadata_prepares_without_scans_and_keeps_both_union_profiles() {
    let definition = parse_query(
        "SELECT CDec(3)/CDbl(2) AS V UNION ALL SELECT CLng(2)",
        Default::default(),
    )
    .unwrap();
    let source = Named {
        definition: definition.query().clone(),
        scans: Cell::new(0),
        rows_allowed: Cell::new(false),
        empty: MemorySource::default(),
    };
    let parsed = parse_query(
        "SELECT VarType(V) AS K, V FROM Quotients",
        Default::default(),
    )
    .unwrap();
    let plan = prepare_with_options(parsed.query(), &source, Default::default()).unwrap();
    assert_eq!(source.scans.get(), 0);
    source.rows_allowed.set(true);
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
        assert_eq!(result.rows().len(), 2);
        for row in result.rows() {
            assert_eq!(row[0], Value::Int16(if declared { 5 } else { 14 }));
        }
    }
}

#[test]
fn null_retained_decimal_pairs_and_operational_errors_keep_existing_boundaries() {
    let source = MemorySource::default();
    for expression in ["CDec(3)/CSng(2)", "CDec(3)/CInt(2)", "CDec(3)/CDec(2)"] {
        let parsed = parse_query(&format!("SELECT {expression} AS V"), Default::default()).unwrap();
        let plan = prepare_with_options(parsed.query(), &source, Default::default()).unwrap();
        assert_eq!(
            scalar_type(plan.output_types().unwrap()[0]),
            Some(ScalarType::Decimal)
        );
        for declared in [false, true] {
            assert!(matches!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral: declared,
                        number_format: Some(NumberFormat::Invariant),
                        ..Default::default()
                    }
                )
                .unwrap()
                .rows()[0][0],
                Value::Decimal(_)
            ));
        }
    }
    for (sql, expected) in [
        ("SELECT CDec(0)/CDbl(0)", EvalError::Overflow),
        ("SELECT CDec(1)/CDbl(0)", EvalError::DivisionByZero),
        (
            "SELECT IIf(True,CDec(3)/CDbl(2),1/0)",
            EvalError::DivisionByZero,
        ),
        ("SELECT CDec(1)/CVErr(7)", EvalError::TypeMismatch),
        ("SELECT CDec(1)/'2'", EvalError::NumberFormatRequired),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare_with_options(parsed.query(), &source, Default::default()).unwrap();
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::Evaluation(expected))
        );
    }
    let parsed = parse_query("SELECT NULL/CDbl(0),CDec(1)/NULL", Default::default()).unwrap();
    let plan = prepare_with_options(parsed.query(), &source, Default::default()).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![Value::Null, Value::Null]]
    );
}

#[test]
fn fixed_named_and_discovered_crosstabs_retain_profiled_cell_schemas() {
    let prefix = concat!(
        "TRANSFORM Sum(CDec(Id)/CDbl(2)) SELECT G FROM ",
        "(SELECT 1 AS G,1 AS H,3 AS Id UNION ALL SELECT 1,2,5) AS D ",
        "GROUP BY G PIVOT H"
    );
    for fixed in [true, false] {
        let sql = if fixed {
            format!("{prefix} IN (1,2)")
        } else {
            prefix.to_owned()
        };
        let definition = parse_query(&sql, Default::default()).unwrap();
        let source = Named {
            definition: definition.query().clone(),
            scans: Cell::new(0),
            rows_allowed: Cell::new(false),
            empty: MemorySource::default(),
        };
        let parsed = parse_query(
            "SELECT [1] AS V FROM Quotients UNION ALL SELECT CLng(2)",
            Default::default(),
        )
        .unwrap();
        if fixed {
            let plan = prepare_with_options(parsed.query(), &source, Default::default()).unwrap();
            assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic]);
        }
        assert_eq!(source.scans.get(), 0);
        source.rows_allowed.set(true);
        for declared in [false, true] {
            let options = ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            };
            let result = tablecraft_query::execute_with_schema_discovery(
                parsed.query(),
                &source,
                &[],
                options,
            )
            .unwrap();
            assert_eq!(
                scalar_type(result.output_types()[0]),
                Some(if declared {
                    ScalarType::Double
                } else {
                    ScalarType::Decimal
                })
            );
            for row in result.rows() {
                assert!(if declared {
                    matches!(row[0], ExpressionValue::Value(Value::Double(_)))
                } else {
                    matches!(row[0], ExpressionValue::Value(Value::Decimal(_)))
                });
            }
        }
    }
}

#[test]
fn unknown_categories_do_not_block_category_independent_division() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT VarType(CDec(3)/First(CDbl(2))) AS K",
        Default::default(),
    )
    .unwrap();
    let plan = prepare_with_options(parsed.query(), &source, Default::default()).unwrap();
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
            [vec![Value::Int16(if declared { 5 } else { 14 })]]
        );
    }
}

#[test]
fn scalar_and_correlated_subqueries_preserve_profile_specific_union_promotions() {
    let source = MemorySource::default();
    for sql in [
        "SELECT (SELECT CDec(3)/CDbl(2)) AS V UNION ALL SELECT CLng(2)",
        concat!(
            "SELECT IIf(True,(SELECT CDec(Q.N)/CDbl(2)),CDec(0)) AS V ",
            "FROM (SELECT 3 AS N) AS Q UNION ALL SELECT CLng(2)"
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare_with_options(parsed.query(), &source, Default::default()).unwrap();
        assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic]);
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
            assert_eq!(result.rows().len(), 2);
            for row in result.rows() {
                assert!(if declared {
                    matches!(row[0], Value::Double(_))
                } else {
                    matches!(row[0], Value::Decimal(_))
                });
            }
        }
    }
}

#[test]
fn constant_heading_arithmetic_uses_captured_preparation_profile_and_checks_runtime() {
    let make = |expression: &str| {
        parse_query(&format!(
            "TRANSFORM Sum(N) SELECT G FROM (SELECT 1 AS G,3 AS N) AS D GROUP BY G PIVOT 0 IN (CLng({expression}))"
        ), Default::default()).unwrap().query().clone()
    };
    let source = Named {
        definition: make("CDec(1)/CDbl(1e100)"),
        scans: Cell::new(0),
        rows_allowed: Cell::new(false),
        empty: MemorySource::default(),
    };
    let parsed = parse_query("SELECT [0] FROM Quotients", Default::default()).unwrap();
    assert_eq!(
        prepare_with_options(parsed.query(), &source, Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::Overflow))
    );
    let plan = prepare_with_options(
        parsed.query(),
        &source,
        PreparationOptions {
            declared_arithmetic: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(source.scans.get(), 0);
    source.rows_allowed.set(true);
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![Value::Int64(3)]]
    );
    // Expressions remain executable and names remain checked. A different
    // runtime mode does not replace the captured metadata heading contract.
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::Evaluation(EvalError::Overflow))
    );

    let source = Named {
        definition: make("CDec(0)/CDbl(0)"),
        scans: Cell::new(0),
        rows_allowed: Cell::new(false),
        empty: MemorySource::default(),
    };
    for (declared, expected) in [
        (false, EvalError::DivisionByZero),
        (true, EvalError::Overflow),
    ] {
        assert_eq!(
            prepare_with_options(
                parsed.query(),
                &source,
                PreparationOptions {
                    declared_arithmetic: declared,
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::Evaluation(expected))
        );
        assert_eq!(source.scans.get(), 0);
    }
}
