use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Single, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}

fn scalar(kind: ScalarType, nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(kind),
        nullable,
    }
}

fn samples() -> Vec<(ScalarType, Value)> {
    vec![
        (ScalarType::Boolean, Value::Boolean(true)),
        (ScalarType::Byte, Value::Byte(2)),
        (ScalarType::Int16, Value::Int16(2)),
        (ScalarType::Int32, Value::Int32(2)),
        (ScalarType::Int64, Value::Int64(2)),
        (ScalarType::Single, Value::Single(Single::from_float(2.5))),
        (ScalarType::Double, Value::Double(Double::from_float(2.5))),
        (
            ScalarType::Currency,
            Value::Currency(Currency::from_scaled(25_000)),
        ),
        (
            ScalarType::Decimal,
            Value::Decimal(Decimal::new(25, 1).unwrap()),
        ),
        (
            ScalarType::DateTime,
            Value::DateTime(LegacyDateTime::from_ole_days(2.5).unwrap()),
        ),
        (ScalarType::Text, Value::Text("2.5".into())),
    ]
}

fn assert_accepts(output: OutputType, value: &Value) {
    match output {
        OutputType::Scalar {
            data_type,
            nullable,
        } => {
            assert!(nullable || *value != Value::Null);
            assert!(
                data_type.validate_value(value).is_ok(),
                "{output:?}: {value:?}"
            );
        }
        OutputType::Null => assert_eq!(*value, Value::Null),
        OutputType::Dynamic => {}
    }
}

#[test]
fn all_supported_unary_and_binary_families_match_runtime_carriers_and_nullability() {
    for nullable in [false, true] {
        for (left_kind, left) in samples() {
            for (right_kind, right) in samples() {
                let mut rows = vec![vec![left.clone(), right.clone()]];
                if nullable {
                    rows.extend([
                        vec![Value::Null, right],
                        vec![left.clone(), Value::Null],
                        vec![Value::Null, Value::Null],
                    ]);
                }
                let source = MemorySource::new(vec![
                    MemoryTable::new(
                        "T".into(),
                        vec![
                            Column::new("A".into(), left_kind, nullable).unwrap(),
                            Column::new("B".into(), right_kind, nullable).unwrap(),
                        ],
                        rows,
                    )
                    .unwrap(),
                ])
                .unwrap();

                for operator in ["And", "Or", "Xor", "Eqv", "Imp"] {
                    let query = syntax(&format!("SELECT A {operator} B,Not A FROM T"));
                    let plan = prepare(&query, &source).unwrap();
                    let result = plan
                        .execute(
                            &source,
                            &[],
                            ExecutionOptions {
                                number_format: Some(NumberFormat::EnUs),
                                ..Default::default()
                            },
                        )
                        .unwrap();
                    assert_eq!(result.output_types(), plan.output_types().unwrap());
                    for row in result.rows() {
                        for (output, value) in result.output_types().iter().zip(row) {
                            assert_accepts(*output, value);
                            if let OutputType::Scalar {
                                nullable: actual, ..
                            } = output
                            {
                                assert_eq!(*actual, nullable);
                            } else {
                                assert!(
                                    nullable,
                                    "nonnullable known operands have a stable carrier"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

struct NoScan {
    columns: Vec<Column>,
    named: Option<Query>,
}

impl Source for NoScan {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(if name.eq_ignore_ascii_case("V") {
            self.named.as_ref()
        } else {
            None
        })
    }

    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("logical metadata must not read records")
    }
}

#[test]
fn binding_known_and_unsupported_types_is_inert_and_drops_changed_constraints() {
    let source = NoScan {
        named: None,
        columns: vec![
            Column::new("B".into(), ScalarType::Byte, false).unwrap(),
            Column::with_type("N".into(), TypeDescriptor::decimal(3, 1).unwrap(), true).unwrap(),
            Column::new("G".into(), ScalarType::Guid, false).unwrap(),
            Column::new("X".into(), ScalarType::Binary, false).unwrap(),
            Column::new("D".into(), ScalarType::DateTimeExtended, false).unwrap(),
        ],
    };
    let query = syntax(
        "SELECT Not B,Not N,B Or NULL,NULL And B,N Xor NULL,NULL Eqv N,Not G,G Or NULL,Not X,Not D,Not CVErr(1) FROM T",
    );
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [
            scalar(ScalarType::Byte, false),
            scalar(ScalarType::Int32, true),
            scalar(ScalarType::Byte, true),
            scalar(ScalarType::Byte, true),
            OutputType::Null,
            OutputType::Null,
            OutputType::Dynamic,
            OutputType::Dynamic,
            OutputType::Dynamic,
            OutputType::Dynamic,
            OutputType::Dynamic,
        ]
    );
}

#[test]
fn logical_result_metadata_survives_derived_union_aggregate_and_empty_sources() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Not A.N FROM (SELECT CByte(1) AS N) AS A",
            scalar(ScalarType::Byte, false),
        ),
        (
            "SELECT A.N FROM (SELECT Not CByte(1) AS N UNION ALL SELECT CInt(-2)) AS A",
            scalar(ScalarType::Int16, false),
        ),
        ("SELECT Min(Not CByte(1))", scalar(ScalarType::Byte, true)),
        (
            "SELECT (SELECT Not CByte(1) WHERE False)",
            scalar(ScalarType::Byte, true),
        ),
        (
            "SELECT Not CByte(1) WHERE False",
            scalar(ScalarType::Byte, false),
        ),
        ("SELECT NULL Xor CLng(1)", OutputType::Null),
        ("SELECT NULL Eqv CByte(1)", OutputType::Null),
        ("SELECT NULL Or NULL", OutputType::Null),
    ] {
        let query = syntax(sql);
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [expected], "{sql}");
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert_eq!(result.output_types(), [expected], "{sql}");
        for row in result.rows() {
            assert_accepts(expected, &row[0]);
        }
        if sql.contains("UNION ALL") {
            assert_eq!(
                result.rows(),
                [vec![Value::Int16(254)], vec![Value::Int16(-2)]]
            );
        }
    }
}

#[test]
fn successful_metadata_never_suppresses_conversion_or_eager_runtime_failures() {
    let source = MemorySource::default();
    for (sql, expected_type, expected_error) in [
        (
            "SELECT Not '1'",
            scalar(ScalarType::Int32, false),
            EvalError::NumberFormatRequired,
        ),
        (
            "SELECT Not CDbl(2147483648)",
            scalar(ScalarType::Int32, false),
            EvalError::Overflow,
        ),
        (
            "SELECT CByte(0) And CDbl(2147483648)",
            scalar(ScalarType::Int32, false),
            EvalError::Overflow,
        ),
        (
            "SELECT NULL Xor CDbl(2147483648)",
            OutputType::Null,
            EvalError::Overflow,
        ),
        (
            "SELECT NULL Eqv (1/0)",
            OutputType::Null,
            EvalError::DivisionByZero,
        ),
    ] {
        let query = syntax(sql);
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [expected_type], "{sql}");
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(expected_error)),
            "{sql}"
        );
    }
}

#[test]
fn nullable_carrier_changes_stay_dynamic_unless_null_results_remove_the_branch() {
    let source = NoScan {
        named: None,
        columns: vec![
            Column::new("B".into(), ScalarType::Byte, true).unwrap(),
            Column::new("F".into(), ScalarType::Boolean, true).unwrap(),
            Column::new("S".into(), ScalarType::Single, true).unwrap(),
            Column::new("L".into(), ScalarType::Int64, true).unwrap(),
        ],
    };
    for (expression, expected) in [
        ("B And F", OutputType::Dynamic),
        ("B Or F", OutputType::Dynamic),
        ("B Imp F", OutputType::Dynamic),
        ("S And B", OutputType::Dynamic),
        ("L Or S", OutputType::Dynamic),
        ("B Xor F", scalar(ScalarType::Int16, true)),
        ("B Eqv F", scalar(ScalarType::Int16, true)),
        ("S Xor B", scalar(ScalarType::Int32, true)),
        ("L Eqv S", scalar(ScalarType::Int64, true)),
        ("B Or B", scalar(ScalarType::Byte, true)),
        ("F And F", scalar(ScalarType::Boolean, true)),
        ("Not S", scalar(ScalarType::Int32, true)),
    ] {
        let query = syntax(&format!("SELECT {expression} FROM T"));
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [expected], "{expression}");
    }
}

#[test]
fn named_logical_metadata_and_text_conversion_metadata_require_no_scans_or_policy() {
    let source = NoScan {
        named: Some(syntax("SELECT Not B AS N,Not Txt AS L FROM T")),
        columns: vec![
            Column::new("B".into(), ScalarType::Byte, false).unwrap(),
            Column::with_type("Txt".into(), TypeDescriptor::text(3).unwrap(), true).unwrap(),
        ],
    };
    let query = syntax("SELECT N,L FROM V");
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [
            scalar(ScalarType::Byte, false),
            scalar(ScalarType::Int32, true)
        ]
    );
}
