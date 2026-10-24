use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::parse_query;

struct Tracked {
    data: MemorySource,
    scans: Cell<usize>,
}

impl Source for Tracked {
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
}

fn source(rows: Vec<Vec<Value>>) -> Tracked {
    Tracked {
        data: MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("Flag".into(), ScalarType::Boolean, false).unwrap()],
                rows,
            )
            .unwrap(),
        ])
        .unwrap(),
        scans: Cell::new(0),
    }
}

fn rows() -> Vec<Vec<Value>> {
    vec![vec![Value::Boolean(true)], vec![Value::Boolean(false)]]
}

fn scalar(kind: ScalarType) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(kind),
        nullable: true,
    }
}

#[test]
fn selector_family_matrix_maps_successful_accumulator_carriers_before_merging() {
    // Independent expected accumulator families, not production inference calls.
    let samples = [
        ("True", Some(ScalarType::Int64), Some(ScalarType::Double)),
        (
            "CByte(2)",
            Some(ScalarType::Int64),
            Some(ScalarType::Double),
        ),
        ("CInt(2)", Some(ScalarType::Int64), Some(ScalarType::Double)),
        ("CLng(2)", Some(ScalarType::Int64), Some(ScalarType::Double)),
        (
            "CLngLng(2)",
            Some(ScalarType::Int64),
            Some(ScalarType::Double),
        ),
        (
            "CSng(2)",
            Some(ScalarType::Double),
            Some(ScalarType::Double),
        ),
        (
            "CDbl(2)",
            Some(ScalarType::Double),
            Some(ScalarType::Double),
        ),
        (
            "CCur(2)",
            Some(ScalarType::Currency),
            Some(ScalarType::Double),
        ),
        (
            "CDec(2)",
            Some(ScalarType::Decimal),
            Some(ScalarType::Decimal),
        ),
        ("CDate(2)", None, Some(ScalarType::Double)),
    ];
    let source = source(rows());

    for (left, left_sum, left_mean) in samples {
        for (right, right_sum, right_mean) in samples {
            for (aggregate, a, b) in [("Sum", left_sum, right_sum), ("Avg", left_mean, right_mean)]
            {
                for selector in [
                    format!("IIf(Flag,{left},{right})"),
                    format!("Switch(Flag,{left},True,{right})"),
                    format!("Choose(IIf(Flag,1,2),{left},{right})"),
                    format!("CVar(IIf(Flag,{left},{right}))"),
                ] {
                    let sql = format!("SELECT {aggregate}({selector}) FROM T");
                    let query = parse_query(&sql, Default::default()).unwrap().into_query();
                    let scans = source.scans.get();
                    let plan = prepare(&query, &source).unwrap();
                    assert_eq!(source.scans.get(), scans, "preparation scanned: {sql}");
                    let expected = match (a, b) {
                        (Some(a), Some(b)) if a == b => scalar(a),
                        _ => OutputType::Dynamic,
                    };
                    assert_eq!(plan.output_types().unwrap(), [expected], "{sql}");

                    for declared_integral in [false, true] {
                        let result = plan.execute(
                            &source,
                            &[],
                            ExecutionOptions {
                                declared_integral,
                                ..Default::default()
                            },
                        );
                        if let OutputType::Scalar { data_type, .. } = expected {
                            let result = result.unwrap_or_else(|error| panic!("{sql}: {error:?}"));
                            assert!(
                                data_type.validate_value(&result.rows()[0][0]).is_ok(),
                                "{sql}"
                            );
                            assert_ne!(result.rows()[0][0], Value::Null, "{sql}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn established_aggregate_schema_survives_derived_and_union_consumers() {
    let source = source(rows());
    for sql in [
        "SELECT S FROM (SELECT Sum(IIf(Flag,CInt(1),CLng(2))) AS S FROM T) AS D",
        "SELECT Sum(IIf(Flag,CInt(1),CLng(2))) AS S FROM T UNION ALL SELECT CLngLng(4)",
        "SELECT Sum(S) FROM (SELECT Sum(IIf(Flag,CInt(1),CLng(2))) AS S FROM T) AS D",
    ] {
        let query = parse_query(sql, Default::default()).unwrap().into_query();
        let scans = source.scans.get();
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(source.scans.get(), scans);
        assert_eq!(
            plan.output_types().unwrap(),
            [scalar(ScalarType::Int64)],
            "{sql}"
        );
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert_eq!(result.rows()[0], [Value::Int64(3)], "{sql}");
    }

    // The heterogeneous row field itself still has no stable carrier. No rows
    // may be inspected to guess the outer aggregate's input alternatives.
    let query = parse_query(
        "SELECT Sum(V) FROM (SELECT IIf(Flag,CInt(1),CLng(2)) AS V FROM T) AS D",
        Default::default(),
    )
    .unwrap()
    .into_query();
    let scans = source.scans.get();
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(source.scans.get(), scans);
    assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic]);
}

#[test]
fn null_empty_and_known_control_results_keep_runtime_failures_separate() {
    for input_rows in [rows(), vec![]] {
        let empty = input_rows.is_empty();
        let source = source(input_rows);
        for (expression, expected) in [
            ("Sum(IIf(Flag,CInt(1),Null))", scalar(ScalarType::Int64)),
            (
                "Avg(Switch(False,CDec(2),True,CLng(4)))",
                scalar(ScalarType::Double),
            ),
            ("Sum(Choose(0,CInt(2),CDec(3)))", OutputType::Null),
            ("Sum(IIf(True,Null,CInt(2)))", OutputType::Null),
            ("Sum(IIf(True,CInt(2),'bad'))", scalar(ScalarType::Int64)),
        ] {
            let sql = format!("SELECT {expression} FROM T");
            let query = parse_query(&sql, Default::default()).unwrap().into_query();
            let plan = prepare(&query, &source).unwrap();
            assert_eq!(plan.output_types().unwrap(), [expected], "{sql}");
            let result = plan.execute(&source, &[], Default::default()).unwrap();
            if empty || expected == OutputType::Null {
                assert_eq!(result.rows()[0], [Value::Null]);
            }
        }
    }

    let source = source(rows());
    for expression in [
        "Sum(IIf(True,CInt(2),1/0))",
        "Avg(Switch(True,CLng(2),False,1/0))",
    ] {
        let query = parse_query(&format!("SELECT {expression} FROM T"), Default::default())
            .unwrap()
            .into_query();
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert!(matches!(
            plan.execute(&source, &[], Default::default()),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        ));
        source.scans.set(0);
    }
}

#[test]
fn unknown_and_unsupported_alternatives_do_not_acquire_numeric_descriptors() {
    let source = source(rows());
    for expression in [
        "Sum(IIf(Flag,CCur(1),CInt(2)))",
        "Avg(IIf(Flag,CDec(1),CDbl(2)))",
        "Sum(IIf(Flag,CInt(1),'2'))",
        "Sum(IIf(Flag,CInt(1),CVErr(7)))",
        "Sum(IIf(Flag,CInt(1),CDate(2)))",
        "Avg(IIf(Flag,CLng(1),CVErr(7)))",
    ] {
        let query = parse_query(&format!("SELECT {expression} FROM T"), Default::default())
            .unwrap()
            .into_query();
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(
            plan.output_types().unwrap(),
            [OutputType::Dynamic],
            "{expression}"
        );
    }
    assert_eq!(source.scans.get(), 0);
}
