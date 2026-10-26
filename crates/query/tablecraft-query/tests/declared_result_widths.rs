//! Scalar overflow refusal makes the successful selected carrier a guarantee.
use tablecraft_core::values::{Currency, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, OutputType, PreparationOptions, QueryError, prepare_with_options,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::parse_query;

fn kind(output: OutputType) -> Option<ScalarType> {
    match output {
        OutputType::Scalar { data_type, .. } => Some(data_type.scalar_type()),
        OutputType::Null | OutputType::Dynamic => None,
    }
}

#[test]
fn selected_scalar_carriers_are_precise_only_in_the_declared_execution_profile() {
    let source = MemorySource::default();
    for (expression, expected) in [
        ("CByte(1)+CByte(2)", ScalarType::Byte),
        ("CBool(True)+CByte(2)", ScalarType::Int16),
        ("CInt(1)+CInt(2)", ScalarType::Int16),
        ("CInt(1)-CInt(2)", ScalarType::Int16),
        ("CLng(1)*CInt(2)", ScalarType::Int32),
        ("CLng(4)\\CInt(2)", ScalarType::Int32),
        ("CInt(4)\\CDbl(2)", ScalarType::Int16),
        ("CSng(1)+CInt(2)", ScalarType::Single),
        ("CSng(1)*CSng(2)", ScalarType::Single),
        ("CSng(1)/CSng(2)", ScalarType::Single),
        ("CDate(1)+CInt(2)", ScalarType::DateTime),
        ("CInt(1)-CDate(2)", ScalarType::DateTime),
        ("-CInt(1)", ScalarType::Int16),
        ("-CLng(1)", ScalarType::Int32),
        ("-CDate(1)", ScalarType::DateTime),
    ] {
        for prepare_declared in [false, true] {
            let sql = format!("SELECT {expression} AS V");
            let syntax = parse_query(&sql, Default::default()).unwrap();
            let plan = prepare_with_options(
                syntax.query(),
                &source,
                PreparationOptions {
                    declared_arithmetic: prepare_declared,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic], "{sql}");

            for execute_declared in [false, true] {
                let result = plan
                    .execute(
                        &source,
                        &[],
                        ExecutionOptions {
                            declared_integral: execute_declared,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert_eq!(
                    kind(result.output_types()[0]),
                    execute_declared.then_some(expected),
                    "{sql}, prepare={prepare_declared}, execute={execute_declared}"
                );
                assert_eq!(
                    ScalarType::of(&result.rows()[0][0]),
                    Some(expected),
                    "{sql}"
                );
            }
        }
    }
}

#[test]
fn scalar_arithmetic_identity_casts_no_longer_change_declared_union_conversion() {
    let source = MemorySource::default();
    for arithmetic in ["CInt(1)+CInt(2)", "-CInt(-3)", "CLng(6)\\CInt(2)"] {
        for reverse in [false, true] {
            let arms = if reverse {
                format!("SELECT CCur(4) AS V UNION ALL SELECT {arithmetic}")
            } else {
                format!("SELECT {arithmetic} AS V UNION ALL SELECT CCur(4)")
            };
            let syntax = parse_query(
                &format!("SELECT V,VarType(V) FROM ({arms}) Q"),
                Default::default(),
            )
            .unwrap();
            let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
            let result = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            let mut amounts = [3, 4];
            if reverse {
                amounts.reverse();
            }
            let expected = amounts.map(|amount| {
                vec![
                    Value::Currency(Currency::from_scaled(amount * 10_000)),
                    Value::Int16(6),
                ]
            });
            assert_eq!(result.rows(), expected, "{arms}");

            let ordinary = plan.execute(&source, &[], Default::default()).unwrap();
            assert!(matches!(
                ordinary.rows()[usize::from(reverse)][0],
                Value::Int16(_) | Value::Int32(_)
            ));
        }
    }
}

#[test]
fn variant_widening_stays_dynamic_and_scalar_overflow_still_fails() {
    let source = MemorySource::default();
    for expression in [
        "CVar(CInt(1))+CInt(2)",
        "CInt(1)+CVar(CInt(2))",
        "-CVar(CInt(1))",
        "CSng(1)*CVar(CInt(2))",
        "CDate(1)+CVar(CInt(2))",
    ] {
        let syntax = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.output_types(), [OutputType::Dynamic], "{expression}");
    }

    for expression in [
        "CByte(255)+CByte(1)",
        "CInt(32767)+CInt(1)",
        "CLng(2147483647)*CLng(2)",
        "-CInt(-32768)",
        "CInt(-32768)\\CInt(-1)",
        "CSng(3e38)*CSng(2)",
        "CDate(2958465)+CInt(1)",
    ] {
        let syntax = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
        assert!(
            matches!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral: true,
                        ..Default::default()
                    }
                ),
                Err(QueryError::Evaluation(EvalError::Overflow))
            ),
            "{expression}"
        );
        assert!(
            plan.execute(&source, &[], Default::default()).is_ok(),
            "{expression}"
        );
    }
}

#[test]
fn nullable_and_empty_sources_keep_static_widths_without_reading_to_infer_them() {
    for rows in [vec![], vec![vec![Value::Null], vec![Value::Int16(2)]]] {
        let source = MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("N".into(), ScalarType::Int16, true).unwrap()],
                rows,
            )
            .unwrap(),
        ])
        .unwrap();
        let syntax = parse_query(
            "SELECT N+N AS Total,-N AS Negative FROM T",
            Default::default(),
        )
        .unwrap();
        let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
        assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic; 2]);
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.output_types(),
            [OutputType::Scalar {
                data_type: TypeDescriptor::new(ScalarType::Int16),
                nullable: true,
            }; 2]
        );
        if !result.rows().is_empty() {
            assert_eq!(
                result.rows(),
                [
                    vec![Value::Null, Value::Null],
                    vec![Value::Int16(4), Value::Int16(-2)],
                ]
            );
        }
    }
}
