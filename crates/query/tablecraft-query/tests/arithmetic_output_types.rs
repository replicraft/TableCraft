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
        (ScalarType::Single, Value::Single(Single::from_float(2.0))),
        (ScalarType::Double, Value::Double(Double::from_float(2.0))),
        (
            ScalarType::Currency,
            Value::Currency(Currency::from_scaled(20_000)),
        ),
        (
            ScalarType::Decimal,
            Value::Decimal(Decimal::new(200, 2).unwrap()),
        ),
        (
            ScalarType::DateTime,
            Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
        ),
        (ScalarType::Text, Value::Text("2".into())),
    ]
}

#[test]
fn every_claimed_binary_descriptor_accepts_successful_runtime_carriers() {
    for (left_kind, left) in samples() {
        for (right_kind, right) in samples() {
            let source = MemorySource::new(vec![
                MemoryTable::new(
                    "T".into(),
                    vec![
                        Column::new("A".into(), left_kind, true).unwrap(),
                        Column::new("B".into(), right_kind, true).unwrap(),
                    ],
                    vec![
                        vec![left.clone(), right.clone()],
                        vec![Value::Null, right.clone()],
                        vec![left.clone(), Value::Null],
                    ],
                )
                .unwrap(),
            ])
            .unwrap();
            for operator in ["+", "-", "*", "/", "\\", "Mod", "^"] {
                let sql = format!("SELECT A {operator} B FROM T");
                let plan = prepare(&syntax(&sql), &source).unwrap();
                let options = ExecutionOptions {
                    number_format: Some(NumberFormat::EnUs),
                    ..Default::default()
                };
                let result = plan.execute(&source, &[], options).unwrap();
                assert_eq!(result.output_types(), plan.output_types().unwrap());
                for row in result.rows() {
                    match result.output_types()[0] {
                        OutputType::Scalar {
                            data_type,
                            nullable,
                        } => {
                            assert!(
                                nullable || row[0] != Value::Null,
                                "{left_kind:?} {operator} {right_kind:?}"
                            );
                            assert!(
                                data_type.validate_value(&row[0]).is_ok(),
                                "{left_kind:?} {operator} {right_kind:?}: {:?}",
                                row[0]
                            );
                        }
                        OutputType::Null => assert_eq!(row[0], Value::Null),
                        OutputType::Dynamic => {}
                    }
                }
            }
        }
    }
}

struct NoScan {
    columns: Vec<Column>,
}
impl Source for NoScan {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("arithmetic inference must not read records")
    }
}

#[test]
fn stable_types_bind_without_rows_and_drop_operand_value_constraints() {
    let source = NoScan {
        columns: vec![
            Column::new("Wide".into(), ScalarType::Int64, false).unwrap(),
            Column::with_type(
                "Fixed".into(),
                TypeDescriptor::decimal(4, 2).unwrap(),
                false,
            )
            .unwrap(),
            Column::with_type("Label".into(), TypeDescriptor::text(2).unwrap(), false).unwrap(),
        ],
    };
    let query =
        syntax("SELECT Wide+Wide,Wide/Wide,Fixed*Wide,Label+Label,Label-Wide,Wide Mod Wide FROM T");
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [
            scalar(ScalarType::Int64, false),
            scalar(ScalarType::Double, false),
            scalar(ScalarType::Decimal, false),
            scalar(ScalarType::Text, false),
            scalar(ScalarType::Double, false),
            scalar(ScalarType::Int64, false),
        ]
    );
    let invalid = syntax("SELECT Label*'invalid',Wide/0,Null+(Wide/0) FROM T");
    assert_eq!(
        prepare(&invalid, &source).unwrap().output_types().unwrap(),
        [
            scalar(ScalarType::Double, false),
            scalar(ScalarType::Double, false),
            OutputType::Null,
        ]
    );
}

#[test]
fn widening_and_unsupported_operand_families_remain_dynamic() {
    let source = MemorySource::new(vec![]).unwrap();
    for expression in [
        "CByte(1)+CByte(2)",
        "CInt(1)-CInt(2)",
        "CLng(1)*CLng(2)",
        "CSng(1)+CByte(1)",
        "CSng(1)/CSng(1)",
        "CDate(1)+CInt(1)",
        "CInt(1)\\CInt(1)",
        "CLng(1)\\CLng(1)",
        "GUIDFromString('{00000000-0000-0000-0000-000000000000}')+1",
        "CVErr(1)+1",
    ] {
        let sql = syntax(&format!("SELECT {expression}"));
        assert_eq!(
            prepare(&sql, &source).unwrap().output_types().unwrap(),
            [OutputType::Dynamic],
            "{expression}"
        );
    }
}

#[test]
fn rounded_remainders_keep_widths_and_quotient_overflow_is_not_mislabelled() {
    let source = MemorySource::new(vec![]).unwrap();
    let query = syntax(
        "SELECT CByte(2) Mod CByte(1),CInt(-32768) Mod CInt(-1),CLng(-2147483648) Mod CLng(-1),CInt(1) Mod CDbl(2),CDbl(1) Mod CInt(2),CByte(2)\\CByte(1),CInt(-32768)\\CInt(-1)",
    );
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [
            scalar(ScalarType::Byte, false),
            scalar(ScalarType::Int16, false),
            scalar(ScalarType::Int32, false),
            scalar(ScalarType::Int16, false),
            scalar(ScalarType::Int32, false),
            scalar(ScalarType::Byte, false),
            OutputType::Dynamic,
        ]
    );
    assert_eq!(
        plan.execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![
            Value::Byte(0),
            Value::Int16(0),
            Value::Int32(0),
            Value::Int16(1),
            Value::Int32(1),
            Value::Byte(2),
            Value::Int32(32768),
        ]]
    );
}

#[test]
fn arithmetic_metadata_survives_empty_derived_union_and_aggregate_consumers() {
    let source = MemorySource::new(vec![]).unwrap();
    for (sql, expected) in [
        (
            "SELECT X FROM (SELECT 1+2 AS X) AS Q WHERE False",
            vec![scalar(ScalarType::Int64, false)],
        ),
        (
            "SELECT CDbl(1)+CInt(2) UNION ALL SELECT CSng(4)",
            vec![scalar(ScalarType::Double, false)],
        ),
        (
            "SELECT Sum(X),Avg(X) FROM (SELECT 1+2 AS X UNION ALL SELECT 3+4) AS Q",
            vec![
                scalar(ScalarType::Int64, true),
                scalar(ScalarType::Double, true),
            ],
        ),
        (
            "TRANSFORM Sum(1+1) SELECT CDbl(1)+1 AS G FROM (SELECT 1 AS P UNION ALL SELECT 2) AS Q GROUP BY CDbl(1)+1 PIVOT P IN (1,2)",
            vec![
                scalar(ScalarType::Double, false),
                scalar(ScalarType::Int64, true),
                scalar(ScalarType::Int64, true),
            ],
        ),
    ] {
        let plan = prepare(&syntax(sql), &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), expected);
        assert_eq!(
            plan.execute(&source, &[], Default::default())
                .unwrap()
                .output_types(),
            expected
        );
    }
}

#[test]
fn inferred_types_do_not_evaluate_or_suppress_failures() {
    let source = MemorySource::new(vec![]).unwrap();
    for (sql, expected, error) in [
        (
            "SELECT Null+(1/0)",
            OutputType::Null,
            EvalError::DivisionByZero,
        ),
        (
            "SELECT 9223372036854775807+1",
            scalar(ScalarType::Int64, false),
            EvalError::Overflow,
        ),
        (
            "SELECT '2'*1",
            scalar(ScalarType::Double, false),
            EvalError::NumberFormatRequired,
        ),
    ] {
        let plan = prepare(&syntax(sql), &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [expected]);
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(error))
        );
    }
    let plan = prepare(
        &syntax("SELECT CDbl(1)+1 AS X UNION ALL SELECT CSng(4)"),
        &source,
    )
    .unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [
            vec![Value::Double(Double::from_float(2.0))],
            vec![Value::Double(Double::from_float(4.0))],
        ]
    );
}
