use std::cell::Cell;
use tablecraft_core::values::{
    Currency, Decimal, Double, ExtendedDateTime, LegacyDateTime, Single, Value,
};
use tablecraft_expressions::{EvalError, functions::dates::parsing::DateTextFormat};
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

fn date(calendar: &str) -> Value {
    Value::DateTime(LegacyDateTime::from_calendar(calendar.parse().unwrap()).unwrap())
}

struct MetadataOnly(Column);

impl Source for MetadataOnly {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(std::slice::from_ref(&self.0))
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("DateAdd metadata cannot inspect source records");
    }
}

#[test]
fn converted_date_sources_have_metadata_before_execution_without_policies_or_rows() {
    for kind in [
        ScalarType::Boolean,
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int32,
        ScalarType::Int64,
        ScalarType::Single,
        ScalarType::Double,
        ScalarType::Currency,
        ScalarType::Decimal,
        ScalarType::Text,
        ScalarType::DateTime,
        ScalarType::DateTimeExtended,
    ] {
        for nullable in [false, true] {
            let source = MetadataOnly(Column::new("Raw".into(), kind, nullable).unwrap());
            let expected = if kind == ScalarType::DateTimeExtended {
                ScalarType::DateTimeExtended
            } else {
                ScalarType::DateTime
            };
            let query = syntax("SELECT DateAdd('d',1,Raw) FROM T");
            assert_eq!(
                prepare(&query, &source).unwrap().output_types().unwrap(),
                [scalar(expected, nullable)],
                "{kind:?}/{nullable}"
            );
        }
    }

    let source = MetadataOnly(
        Column::with_type("Raw".into(), TypeDescriptor::text(3).unwrap(), false).unwrap(),
    );
    let query = syntax(concat!(
        "PARAMETERS P TEXT; SELECT DateAdd('d',1,P),",
        "DateAdd('d',1,Raw),DateAdd('d',1,Null) FROM T"
    ));
    assert_eq!(
        prepare(&query, &source).unwrap().output_types().unwrap(),
        [
            scalar(ScalarType::DateTime, true),
            scalar(ScalarType::DateTime, false),
            OutputType::Null,
        ]
    );

    for kind in [ScalarType::Binary, ScalarType::Guid] {
        let source = MetadataOnly(Column::new("Raw".into(), kind, true).unwrap());
        let query = syntax("SELECT DateAdd('d',1,Raw),DateAdd('d',1,CVErr(1)) FROM T");
        assert_eq!(
            prepare(&query, &source).unwrap().output_types().unwrap(),
            [OutputType::Dynamic, OutputType::Dynamic]
        );
    }
}

#[test]
fn converted_and_existing_date_values_satisfy_metadata_without_narrowing_extended_dates() {
    let midnight = date("1900-01-01T00:00:00");
    for (kind, input, expected) in [
        (
            ScalarType::Boolean,
            Value::Boolean(true),
            date("1899-12-30T00:00:00"),
        ),
        (ScalarType::Byte, Value::Byte(1), midnight.clone()),
        (ScalarType::Int16, Value::Int16(1), midnight.clone()),
        (ScalarType::Int32, Value::Int32(1), midnight.clone()),
        (ScalarType::Int64, Value::Int64(1), midnight.clone()),
        (
            ScalarType::Single,
            Value::Single(Single::from_float(1.0)),
            midnight.clone(),
        ),
        (
            ScalarType::Double,
            Value::Double(Double::from_float(1.0)),
            midnight.clone(),
        ),
        (
            ScalarType::Currency,
            Value::Currency(Currency::from_scaled(10_000)),
            midnight.clone(),
        ),
        (
            ScalarType::Decimal,
            Value::Decimal(Decimal::new(100, 2).unwrap()),
            midnight,
        ),
        (
            ScalarType::Text,
            Value::Text("2024-01-31".into()),
            date("2024-02-01T00:00:00"),
        ),
        (
            ScalarType::DateTime,
            Value::DateTime(LegacyDateTime::from_ole_days(-0.25).unwrap()),
            date("1899-12-31T06:00:00"),
        ),
        (
            ScalarType::DateTimeExtended,
            Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 1234567).unwrap()),
            Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 2, 0, 0, 0, 1234567).unwrap()),
        ),
    ] {
        let table = MemoryTable::new(
            "T".into(),
            vec![Column::new("Raw".into(), kind, true).unwrap()],
            vec![vec![input], vec![Value::Null]],
        )
        .unwrap();
        let source = MemorySource::new(vec![table]).unwrap();
        let query = syntax("SELECT DateAdd('d',1,Raw) AS NextDay FROM T");
        let plan = prepare(&query, &source).unwrap();

        let options = ExecutionOptions {
            date_format: Some(DateTextFormat::Iso),
            ..Default::default()
        };
        let result = plan.execute(&source, &[], options).unwrap();
        assert_eq!(
            result.rows(),
            [vec![expected], vec![Value::Null]],
            "{kind:?}"
        );
        assert_eq!(result.output_types(), plan.output_types().unwrap());
        let OutputType::Scalar {
            data_type,
            nullable,
        } = result.output_types()[0]
        else {
            panic!("expected checked DateAdd metadata");
        };
        assert!(nullable);
        for row in result.rows() {
            assert!(data_type.validate_value(&row[0]).is_ok());
        }
    }
}

struct Views {
    tables: MemorySource,
    query: Query,
    scans: Cell<usize>,
}

impl Source for Views {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.tables.columns(table)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("Dates").then_some(&self.query))
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.tables.scan(table, visitor)
    }
}

#[test]
fn named_derived_union_and_aggregate_consumers_keep_converted_date_metadata() {
    let table = MemoryTable::new(
        "T".into(),
        vec![Column::new("Raw".into(), ScalarType::Int16, false).unwrap()],
        vec![vec![Value::Int16(1)]],
    )
    .unwrap();
    let source = Views {
        tables: MemorySource::new(vec![table]).unwrap(),
        query: syntax("SELECT DateAdd('d',1,Raw) AS Stamp FROM T"),
        scans: Cell::new(0),
    };
    for (sql, expected) in [
        (
            "SELECT Stamp FROM Dates",
            scalar(ScalarType::DateTime, false),
        ),
        (
            "SELECT D.Stamp FROM (SELECT Stamp FROM Dates) AS D",
            scalar(ScalarType::DateTime, false),
        ),
        (
            "SELECT Stamp FROM Dates UNION ALL SELECT Null",
            scalar(ScalarType::DateTime, true),
        ),
        (
            "SELECT Avg(Stamp) FROM Dates",
            scalar(ScalarType::Double, true),
        ),
        (
            "SELECT Nz(Stamp) FROM Dates",
            scalar(ScalarType::DateTime, false),
        ),
    ] {
        source.scans.set(0);
        let query = syntax(sql);
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(plan.output_types().unwrap(), [expected], "{sql}");
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert_eq!(result.output_types(), [expected]);
    }
}

#[test]
fn metadata_does_not_evaluate_or_suppress_date_controls_and_conversion_failures() {
    let source = MemorySource::default();
    for (sql, expected_type, error) in [
        (
            "SELECT DateAdd('invalid',1,Null)",
            OutputType::Null,
            EvalError::InvalidDateInterval,
        ),
        (
            "SELECT DateAdd('d',1,'bad date')",
            scalar(ScalarType::DateTime, false),
            EvalError::DateFormatRequired,
        ),
        (
            "SELECT DateAdd('d',1,Null+(1/0))",
            OutputType::Null,
            EvalError::DivisionByZero,
        ),
        (
            "SELECT DateAdd('d',1,CLngLng(9223372036854775807))",
            scalar(ScalarType::DateTime, false),
            EvalError::Overflow,
        ),
    ] {
        let query = syntax(sql);
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [expected_type]);
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(error)),
            "{sql}"
        );
    }
}
