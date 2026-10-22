use std::cell::Cell;
use tablecraft_core::values::{
    Currency, Decimal, Double, ExpressionValue, ExtendedDateTime, LegacyDateTime, Single, Value,
};
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
};
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, execute_with_schema_discovery, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

const DYNAMIC: &str =
    "TRANSFORM Count(*) SELECT True AS G,Count(*) AS Total FROM T GROUP BY True PIVOT P";

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}

fn table_source(kind: ScalarType, values: Vec<Value>) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("P".into(), kind, true).unwrap()],
            values.into_iter().map(|value| vec![value]).collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn options() -> ExecutionOptions<'static> {
    ExecutionOptions {
        number_format: Some(NumberFormat::EnUs),
        date_format: Some(DateTextFormat::Iso),
        ..Default::default()
    }
}

#[test]
fn fixed_point_and_float_pivot_names_use_explicit_formats_and_typed_keys() {
    for (kind, low, high) in [
        (
            ScalarType::Single,
            Value::Single(Single::from_float(-1.25)),
            Value::Single(Single::from_float(1.25)),
        ),
        (
            ScalarType::Double,
            Value::Double(Double::from_float(-1.25)),
            Value::Double(Double::from_float(1.25)),
        ),
        (
            ScalarType::Currency,
            Value::Currency(Currency::from_scaled(-12500)),
            Value::Currency(Currency::from_scaled(12500)),
        ),
        (
            ScalarType::Decimal,
            Value::Decimal(Decimal::new(-12500, 4).unwrap()),
            Value::Decimal(Decimal::new(125, 2).unwrap()),
        ),
    ] {
        let source = table_source(kind, vec![high.clone(), low, high, Value::Null]);
        let plan = prepare(&syntax(DYNAMIC), &source).unwrap();
        for (format, names) in [
            (NumberFormat::EnUs, ["-1.25", "1.25"]),
            (NumberFormat::DeDe, ["-1,25", "1,25"]),
        ] {
            let result = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        number_format: Some(format),
                        ..options()
                    },
                )
                .unwrap();
            assert_eq!(result.columns(), ["G", "Total", names[0], names[1]]);
            assert_eq!(
                result.rows(),
                [vec![
                    Value::Boolean(true),
                    Value::Int64(4),
                    Value::Int64(1),
                    Value::Int64(2)
                ]]
            );
        }
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
}

#[test]
fn legacy_and_extended_dates_pivot_without_converting_their_keys_to_text() {
    for (kind, low, high, names) in [
        (
            ScalarType::DateTime,
            Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
            Value::DateTime(LegacyDateTime::from_ole_days(3.0).unwrap()),
            ["1900-01-01", "1900-01-02"],
        ),
        (
            ScalarType::DateTimeExtended,
            Value::DateTimeExtended(ExtendedDateTime::new(2024, 1, 1, 0, 0, 0, 0).unwrap()),
            Value::DateTimeExtended(ExtendedDateTime::new(2024, 1, 2, 0, 0, 0, 0).unwrap()),
            ["2024-01-01", "2024-01-02"],
        ),
    ] {
        let source = table_source(kind, vec![high.clone(), low, high]);
        let plan = prepare(&syntax(DYNAMIC), &source).unwrap();
        let result = plan.execute(&source, &[], options()).unwrap();
        assert_eq!(result.columns(), ["G", "Total", names[0], names[1]]);
        assert_eq!(
            result.rows(),
            [vec![
                Value::Boolean(true),
                Value::Int64(3),
                Value::Int64(1),
                Value::Int64(2)
            ]]
        );
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(EvalError::DateFormatRequired))
        );
    }
}

struct Named {
    data: MemorySource,
    definition: Query,
    scans: Cell<usize>,
}

impl Source for Named {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "Pivoted").then_some(&self.definition))
    }
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

#[test]
fn contextual_named_schemas_need_explicit_discovery_and_are_not_cached_between_formats() {
    let source = Named {
        data: table_source(
            ScalarType::Currency,
            vec![Value::Currency(Currency::from_scaled(12500))],
        ),
        definition: syntax(
            "TRANSFORM Count(*) SELECT True AS G FROM T GROUP BY True PIVOT P IN (CCur(1.25),CCur(2.5))",
        ),
        scans: Cell::new(0),
    };
    let query = syntax("SELECT * FROM Pivoted");
    assert_eq!(
        prepare(&query, &source).err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(source.scans.get(), 0);
    for (format, names) in [
        (NumberFormat::EnUs, ["1.25", "2.5"]),
        (NumberFormat::DeDe, ["1,25", "2,5"]),
    ] {
        let result = execute_with_schema_discovery(
            &query,
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(format),
                ..options()
            },
        )
        .unwrap();
        assert_eq!(result.columns(), ["G", names[0], names[1]]);
        assert_eq!(
            result.rows()[0],
            vec![Value::Boolean(true), Value::Int64(1), Value::Null]
                .into_iter()
                .map(ExpressionValue::Value)
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(source.scans.get(), 2);
}

#[test]
fn named_date_headings_discover_per_execution_without_scanning_during_preparation() {
    let source = Named {
        data: table_source(
            ScalarType::DateTime,
            vec![Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap())],
        ),
        definition: syntax(DYNAMIC),
        scans: Cell::new(0),
    };
    let query = syntax("SELECT * FROM Pivoted");

    assert_eq!(
        prepare(&query, &source).err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(source.scans.get(), 0);

    for (format, heading) in [
        (DateTextFormat::Iso, "1900-01-01"),
        (DateTextFormat::DeDe, "01.01.1900"),
    ] {
        let result = execute_with_schema_discovery(
            &query,
            &source,
            &[],
            ExecutionOptions {
                date_format: Some(format),
                ..options()
            },
        )
        .unwrap();
        assert_eq!(result.columns(), ["G", "Total", heading]);
        assert_eq!(
            result.rows()[0],
            vec![Value::Boolean(true), Value::Int64(1), Value::Int64(1)]
                .into_iter()
                .map(ExpressionValue::Value)
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(source.scans.get(), 2);
}

#[test]
fn boolean_fixed_headings_bind_without_formats_or_scans_and_preserve_written_order() {
    let source = Named {
        data: table_source(
            ScalarType::Boolean,
            vec![Value::Boolean(true), Value::Boolean(false)],
        ),
        definition: syntax(
            "TRANSFORM Count(*) SELECT True AS G FROM T GROUP BY True PIVOT P IN (True,False)",
        ),
        scans: Cell::new(0),
    };
    let query = syntax("SELECT * FROM Pivoted");
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = plan.execute(&source, &[], Default::default()).unwrap();
    assert_eq!(result.columns(), ["G", "True", "False"]);
    assert_eq!(
        result.rows(),
        [vec![Value::Boolean(true), Value::Int64(1), Value::Int64(1)]]
    );
}

#[test]
fn distinct_typed_keys_cannot_silently_collapse_after_name_rounding() {
    let source = table_source(
        ScalarType::Double,
        vec![
            Value::Double(Double::from_float(1.0)),
            Value::Double(Double::from_float(f64::from_bits(1.0_f64.to_bits() + 1))),
        ],
    );
    let plan = prepare(&syntax(DYNAMIC), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], options()).err(),
        Some(QueryError::AmbiguousName)
    );
    let source = table_source(
        ScalarType::DateTimeExtended,
        vec![
            Value::DateTimeExtended(ExtendedDateTime::new(2024, 1, 1, 0, 0, 0, 1).unwrap()),
            Value::DateTimeExtended(ExtendedDateTime::new(2024, 1, 1, 0, 0, 0, 2).unwrap()),
        ],
    );
    let plan = prepare(&syntax(DYNAMIC), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], options()).err(),
        Some(QueryError::AmbiguousName)
    );
}

#[test]
fn missing_policies_and_excessive_names_fail_even_when_where_rejects_rows() {
    let source = table_source(
        ScalarType::Double,
        vec![Value::Double(Double::from_float(1.0e-300))],
    );
    let query =
        syntax("TRANSFORM Count(*) SELECT True AS G FROM T WHERE False GROUP BY True PIVOT P");
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], options()).err(),
        Some(QueryError::Unsupported)
    );
    let source = table_source(ScalarType::Currency, vec![]);
    let query =
        syntax("TRANSFORM Count(*) SELECT True AS G FROM T GROUP BY True PIVOT P IN (CCur(1.25))");
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
    let query = syntax(DYNAMIC);
    assert!(
        prepare(&query, &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .is_ok()
    );
}

#[test]
fn every_observed_scalar_heading_checkpoint_can_cancel_without_partial_results() {
    let source = table_source(
        ScalarType::Decimal,
        vec![
            Value::Decimal(Decimal::new(125, 2).unwrap()),
            Value::Decimal(Decimal::new(250, 2).unwrap()),
        ],
    );
    let plan = prepare(&syntax(DYNAMIC), &source).unwrap();
    let calls = Cell::new(0);
    let run = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..options()
            },
        )
    };
    assert!(run(usize::MAX).is_ok());
    let count = calls.get();
    for stop in 1..=count {
        assert!(
            matches!(
                run(stop),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
}
