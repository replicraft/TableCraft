//! Successful agreement establishes a carrier without declaring Scalar or Variant.
use tablecraft_core::values::{Currency, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    BindingDepth, ExecutionOptions, OutputType, PreparationOptions, QueryError, QueryResult,
    StreamError, prepare_with_options,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::parse_query;

#[path = "agreement_result_types/boundaries.rs"]
mod boundaries;

fn run(sql: &str, declared: bool) -> Result<QueryResult, QueryError> {
    let source = MemorySource::default();
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare_with_options(
        syntax.query(),
        &source,
        PreparationOptions {
            binding_depth: BindingDepth::new(32).unwrap(),
            ..Default::default()
        },
    )?;
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            declared_integral: declared,
            number_format: Some(NumberFormat::Invariant),
            ..Default::default()
        },
    )
}

#[test]
fn successful_unknown_arithmetic_has_selected_carriers_only_in_the_declared_profile() {
    for (expression, expected) in [
        ("First(CByte(1))+CByte(2)", ScalarType::Byte),
        ("First(CInt(1))+CInt(2)", ScalarType::Int16),
        ("First(CInt(1))-CInt(2)", ScalarType::Int16),
        ("First(CLng(3))*CInt(2)", ScalarType::Int32),
        ("First(CInt(6))\\CInt(2)", ScalarType::Int16),
        ("First(CInt(6))\\CDbl(2)", ScalarType::Int16),
        ("First('6')\\CInt(2)", ScalarType::Int32),
        ("First(CSng(1))+CInt(2)", ScalarType::Single),
        ("First(CSng(6))/CSng(2)", ScalarType::Single),
        ("First(CDate(1))+CInt(2)", ScalarType::DateTime),
        ("CInt(1)-First(CDate(2))", ScalarType::DateTime),
        ("-First(CInt(1))", ScalarType::Int16),
        ("-First(CLng(1))", ScalarType::Int32),
        ("-First(CDate(1))", ScalarType::DateTime),
    ] {
        let sql = format!("SELECT {expression} AS V");
        let ordinary = run(&sql, false).unwrap();
        assert_eq!(ordinary.output_types(), [OutputType::Dynamic], "{sql}");
        let declared = run(&sql, true).unwrap();
        assert_eq!(
            declared.output_types(),
            [OutputType::Scalar {
                data_type: TypeDescriptor::new(expected),
                nullable: true
            }],
            "{sql}"
        );
        assert_eq!(
            ScalarType::of(&declared.rows()[0][0]),
            Some(expected),
            "{sql}"
        );
    }
}

#[test]
fn selected_unknown_results_compose_with_union_conversion_in_both_arm_orders() {
    for expression in [
        "First(CInt(1))+CInt(2)",
        "-First(CInt(-3))",
        "First(CInt(6))\\CInt(2)",
    ] {
        for reverse in [false, true] {
            let arms = if reverse {
                format!("SELECT CCur(4) AS V UNION ALL SELECT {expression}")
            } else {
                format!("SELECT {expression} AS V UNION ALL SELECT CCur(4)")
            };
            let sql = format!("SELECT V,VarType(V) FROM ({arms}) Q");
            let result = run(&sql, true).unwrap();
            let mut amounts = [3, 4];
            if reverse {
                amounts.reverse();
            }
            assert_eq!(
                result.rows(),
                amounts.map(|amount| vec![
                    Value::Currency(Currency::from_scaled(amount * 10_000)),
                    Value::Int16(6),
                ]),
                "{sql}"
            );
            let ordinary = run(&sql, false).unwrap();
            assert!(matches!(
                ordinary.rows()[usize::from(reverse)][0],
                Value::Int16(_)
            ));
        }
    }
}

#[test]
fn carrier_metadata_does_not_assign_a_declaration_or_permit_widening() {
    for expression in [
        "First(CByte(255))+CByte(1)",
        "First(CInt(32767))+CInt(1)",
        "First(CInt(-32768))\\CInt(-1)",
        "First(CSng(3e38))*CInt(2)",
        "First(CDate(2958465))+CInt(1)",
        "-First(CInt(-32768))",
        "Q.V+CInt(32767) FROM (SELECT First(CInt(1))+CInt(1) AS V) Q",
        "'02'=Q.V FROM (SELECT First(CInt(1))+CInt(1) AS V) Q",
    ] {
        assert_eq!(
            run(&format!("SELECT {expression}"), true).err(),
            Some(QueryError::ArithmeticDeclaration),
            "{expression}"
        );
    }
    assert_eq!(
        run("SELECT First(CInt(1))\\CInt(0)", true).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn a_known_variant_still_allows_widening_and_keeps_dynamic_metadata() {
    for expression in [
        "First(CInt(1))+CVar(CInt(2))",
        "CVar(CInt(1))+First(CInt(2))",
        "First(CSng(1))*CVar(CInt(2))",
        "First(CDate(1))+CVar(CInt(2))",
    ] {
        let result = run(&format!("SELECT {expression}"), true).unwrap();
        assert_eq!(result.output_types(), [OutputType::Dynamic]);
    }
    let result = run("SELECT First(CInt(32767))+CVar(CInt(1))", true).unwrap();
    assert_eq!(result.rows(), [vec![Value::Int32(32768)]]);
}

#[test]
fn empty_and_nullable_unknown_outputs_keep_the_same_static_guarantee() {
    for rows in [vec![], vec![vec![Value::Null]], vec![vec![Value::Int16(2)]]] {
        let source = MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("N".into(), ScalarType::Int16, true).unwrap()],
                rows,
            )
            .unwrap(),
        ])
        .unwrap();
        let sql = "SELECT Q.N+CInt(1) AS V,-Q.N AS M FROM (SELECT N FROM T UNION ALL SELECT N FROM T WHERE False) Q";
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare_with_options(
            syntax.query(),
            &source,
            PreparationOptions {
                binding_depth: BindingDepth::new(32).unwrap(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic; 2]);
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
            let expected = if declared {
                OutputType::Scalar {
                    data_type: TypeDescriptor::new(ScalarType::Int16),
                    nullable: true,
                }
            } else {
                OutputType::Dynamic
            };
            assert_eq!(result.output_types(), [expected; 2]);
        }
    }
}

#[test]
fn progressive_aggregate_outputs_keep_the_same_outer_arithmetic_and_refusals() {
    for value in [2, i16::MAX] {
        let source = MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("N".into(), ScalarType::Int16, true).unwrap()],
                vec![vec![Value::Null], vec![Value::Int16(value)]],
            )
            .unwrap(),
        ])
        .unwrap();
        let syntax = parse_query(
            "SELECT Q.N+CInt(1) AS V FROM (SELECT Min(N) AS N FROM T) Q",
            Default::default(),
        )
        .unwrap();
        let plan = prepare_with_options(
            syntax.query(),
            &source,
            PreparationOptions {
                binding_depth: BindingDepth::new(32).unwrap(),
                ..Default::default()
            },
        )
        .unwrap();
        for declared in [false, true] {
            let options = ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            };
            let buffered = plan.execute(&source, &[], options);
            let mut rows = Vec::new();
            let progressive = plan.execute_progressive(&source, &[], options, &mut |row| {
                rows.push(row.to_vec());
                Ok::<(), ()>(())
            });
            if declared && value == i16::MAX {
                assert_eq!(buffered.err(), Some(QueryError::ArithmeticDeclaration));
                assert!(matches!(
                    progressive,
                    Err(StreamError::Query(QueryError::ArithmeticDeclaration))
                ));
                assert!(rows.is_empty());
            } else {
                let expected = buffered.unwrap();
                assert!(progressive.is_ok());
                assert_eq!(rows, expected.rows());
            }
        }
    }
}
