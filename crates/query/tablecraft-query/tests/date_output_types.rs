use std::cell::Cell;
use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::EvalError;
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

struct Views {
    tables: MemorySource,
    view: Query,
    scans: Cell<usize>,
}

impl Source for Views {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.tables.columns(table)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("V").then_some(&self.view))
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

fn source() -> Views {
    let date = Value::DateTime(LegacyDateTime::from_ole_days(1.0).unwrap());
    Views {
        tables: MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![
                    Column::new("D".into(), ScalarType::DateTime, false).unwrap(),
                    Column::new("N".into(), ScalarType::DateTime, true).unwrap(),
                ],
                vec![vec![date, Value::Null]],
            )
            .unwrap(),
        ])
        .unwrap(),
        view: syntax("SELECT Year(NULL) AS Missing,Year(D) AS Y FROM T"),
        scans: Cell::new(0),
    }
}

#[test]
fn null_only_date_results_are_known_without_scans_or_policies() {
    let source = source();
    for expression in [
        "Year(NULL)",
        "Month(NULL)",
        "Day(NULL)",
        "Hour(NULL)",
        "Minute(NULL)",
        "Second(NULL)",
        "Weekday(NULL)",
        "DatePart('m',NULL)",
        "DateDiff('d',NULL,CDate(1))",
        "DateDiff('d',CDate(1),NULL)",
        "DateValue(NULL)",
        "TimeValue(NULL)",
    ] {
        source.scans.set(0);
        let query = syntax(&format!("SELECT {expression}"));
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            plan.output_types().unwrap(),
            [OutputType::Null],
            "{expression}"
        );
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert_eq!(result.rows(), [vec![Value::Null]], "{expression}");
        assert_eq!(result.output_types(), [OutputType::Null]);
    }
}

#[test]
fn source_nullability_propagates_only_from_date_value_operands() {
    let source = source();
    for (expression, kind, nullable) in [
        ("Year(D)", ScalarType::Int16, false),
        ("Year(N)", ScalarType::Int16, true),
        ("Weekday(D)", ScalarType::Int16, false),
        ("Weekday(N)", ScalarType::Int16, true),
        ("DatePart('m',D)", ScalarType::Int16, false),
        ("DatePart('m',N)", ScalarType::Int16, true),
        ("DateDiff('d',D,D)", ScalarType::Int32, false),
        ("DateDiff('d',D,N)", ScalarType::Int32, true),
        ("DateDiff('d',N,D)", ScalarType::Int32, true),
        ("DateValue(D)", ScalarType::DateTime, false),
        ("DateValue(N)", ScalarType::DateTime, true),
        ("TimeValue(D)", ScalarType::DateTime, false),
        ("TimeValue(N)", ScalarType::DateTime, true),
    ] {
        source.scans.set(0);
        let query = syntax(&format!("SELECT {expression} FROM T"));
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            plan.output_types().unwrap(),
            [scalar(kind, nullable)],
            "{expression}"
        );
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert_eq!(result.output_types(), [scalar(kind, nullable)]);
        for row in result.rows() {
            assert!(nullable || row[0] != Value::Null, "{expression}");
            assert!(TypeDescriptor::new(kind).validate_value(&row[0]).is_ok());
        }
    }
}

#[test]
fn constructors_and_strict_conversions_have_nonnull_successful_carriers() {
    let source = source();
    for expression in [
        "DateSerial(2026,10,24)",
        "TimeSerial(1,2,3)",
        "CDate(1)",
        "CVDate(1)",
    ] {
        let query = syntax(&format!("SELECT {expression}"));
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(
            plan.output_types().unwrap(),
            [scalar(ScalarType::DateTime, false)]
        );
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert!(matches!(result.rows()[0][0], Value::DateTime(_)));
    }

    for expression in [
        "CDate(NULL)",
        "CVDate(NULL)",
        "DateSerial(2026,NULL,1)",
        "TimeSerial(NULL,0,0)",
    ] {
        let query = syntax(&format!("SELECT {expression}"));
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(
            plan.output_types().unwrap(),
            [scalar(ScalarType::DateTime, false)]
        );
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(EvalError::InvalidNullConversion)),
            "{expression}"
        );
    }
}

#[test]
fn named_derived_union_and_null_replacement_keep_precise_date_metadata() {
    let source = source();
    for (sql, expected) in [
        ("SELECT Missing FROM V", OutputType::Null),
        (
            "SELECT Q.Missing FROM (SELECT Missing FROM V) AS Q",
            OutputType::Null,
        ),
        (
            "SELECT Missing FROM V UNION ALL SELECT 'kept'",
            scalar(ScalarType::Text, true),
        ),
        (
            "SELECT Nz(Missing,'kept') FROM V",
            scalar(ScalarType::Text, false),
        ),
        (
            "SELECT Nz(Y,'unused') FROM V",
            scalar(ScalarType::Int16, false),
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
fn known_null_metadata_does_not_suppress_eager_or_control_failures() {
    let source = source();
    for (expression, expected) in [
        ("Year(NULL+(1/0))", EvalError::DivisionByZero),
        ("DatePart('invalid',NULL)", EvalError::InvalidDateInterval),
        (
            "DateDiff('invalid',NULL,CDate(1))",
            EvalError::InvalidDateInterval,
        ),
        (
            "DateDiff('d',NULL,CDate('bad'))",
            EvalError::DateFormatRequired,
        ),
        ("Weekday(NULL,1/0)", EvalError::DivisionByZero),
        ("DateValue(NULL+(1/0))", EvalError::DivisionByZero),
    ] {
        source.scans.set(0);
        let query = syntax(&format!("SELECT {expression}"));
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            plan.output_types().unwrap(),
            [OutputType::Null],
            "{expression}"
        );
        assert_eq!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .err(),
            Some(QueryError::Evaluation(expected)),
            "{expression}"
        );
    }
}

#[test]
fn dynamic_sources_keep_possible_null_without_losing_fixed_successful_carriers() {
    let source = source();
    for (expression, expected) in [
        (
            "Year(IIf(True,CDate(1),CVErr(1)))",
            scalar(ScalarType::Int16, true),
        ),
        (
            "DateDiff('d',IIf(True,CDate(1),CVErr(1)),CDate(1))",
            scalar(ScalarType::Int32, true),
        ),
        (
            "DateValue(IIf(True,CDate(1),CVErr(1)))",
            scalar(ScalarType::DateTime, true),
        ),
        (
            "CDate(IIf(True,CDate(1),CVErr(1)))",
            scalar(ScalarType::DateTime, false),
        ),
    ] {
        let query = syntax(&format!("SELECT {expression}"));
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [expected], "{expression}");
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert_eq!(result.output_types(), [expected]);
        assert_ne!(result.rows()[0][0], Value::Null);
    }
}
