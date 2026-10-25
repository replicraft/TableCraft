use super::{BinaryCollation, source};
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare, source::MemorySource,
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

#[test]
fn array_bounds_supply_numeric_metadata_for_common_text_union_conversion() {
    let source = source();
    for (function, expected) in [("LBound", "0"), ("UBound", "1")] {
        let sql = format!("SELECT {function}(Array(1,2)) UNION SELECT '{expected}'");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(NumberFormat::Invariant),
                    collation: Some(&BinaryCollation),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Text(expected.into())]]);
    }
}

#[test]
fn bound_descriptors_widen_existing_numeric_carriers_and_preserve_empty_aggregate_nulls() {
    let source = source();
    for sql in [
        "SELECT LBound(Array(1,2)) UNION ALL SELECT CLngLng(2)",
        "SELECT D.N FROM (SELECT LBound(Array(1,2)) AS N UNION ALL SELECT CLngLng(2)) AS D",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.output_types(), [scalar(ScalarType::Int64, false)]);
        assert_eq!(
            result.rows(),
            [vec![Value::Int64(0)], vec![Value::Int64(2)]]
        );
    }

    for (sql, expected, output) in [
        (
            "SELECT Sum(UBound(Array(N,N))) FROM T UNION ALL SELECT CByte(3)",
            vec![vec![Value::Int64(3)], vec![Value::Int64(3)]],
            scalar(ScalarType::Int64, true),
        ),
        (
            "SELECT First(UBound(Array(N,N))) FROM T WHERE False UNION ALL SELECT CByte(3)",
            vec![vec![Value::Null], vec![Value::Int32(3)]],
            scalar(ScalarType::Int32, true),
        ),
        (
            "SELECT UBound(Array()) WHERE False UNION ALL SELECT CByte(3)",
            vec![vec![Value::Int32(3)]],
            scalar(ScalarType::Int32, false),
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.output_types(), [output], "{sql}");
        assert_eq!(result.rows(), expected, "{sql}");
    }
}

fn scalar(kind: ScalarType, nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(kind),
        nullable,
    }
}

struct Named {
    source: MemorySource,
    query: Query,
    scans: Cell<usize>,
}

impl Source for Named {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.source.columns(name)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("Bounds").then_some(&self.query))
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.source.scan(name, visitor)
    }
}

#[test]
fn named_binary_and_correlated_bounds_keep_types_without_metadata_scans() {
    let source = Named {
        source: source(),
        query: parse_query(
            "SELECT LBound(B) AS Low,UBound(B) AS High FROM T",
            Default::default(),
        )
        .unwrap()
        .into_query(),
        scans: Cell::new(0),
    };
    for sql in [
        "SELECT Low,High FROM Bounds",
        "SELECT D.Low,(SELECT D.High) FROM (SELECT Low,High FROM Bounds) AS D",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let previous_scans = source.scans.get();
        let plan = prepare(parsed.query(), &source).unwrap();
        let types = plan.output_types().unwrap();
        assert_eq!(types[0], scalar(ScalarType::Int32, false));
        assert!(
            matches!(types[1], OutputType::Scalar { data_type, .. } if data_type.scalar_type() == ScalarType::Int32)
        );
        assert_eq!(source.scans.get(), previous_scans);
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert_eq!(
            result.rows(),
            vec![vec![Value::Int32(0), Value::Int32(1)]; 3]
        );
        assert_eq!(source.scans.get(), previous_scans + 1);
    }
}

struct MetadataOnly(MemorySource);

impl Source for MetadataOnly {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.0.columns(name)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("bound inference must not inspect values or execute failing operands")
    }
}

#[test]
fn fixed_success_descriptor_never_evaluates_or_masks_failing_bound_operands() {
    let metadata = MetadataOnly(source());
    for sql in [
        "SELECT LBound(Array(N,1/0)),UBound(B,1/0) FROM T",
        "SELECT UBound(NULL),LBound(Array()) FROM T",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &metadata).unwrap();
        assert_eq!(
            plan.output_types().unwrap(),
            [scalar(ScalarType::Int32, false); 2]
        );
    }

    let source = source();
    for (sql, error) in [
        (
            "SELECT UBound(Array(1/0)) UNION ALL SELECT 1",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT UBound(NULL,1/0) UNION ALL SELECT 1",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT LBound(Array(1),2) UNION ALL SELECT 1",
            EvalError::ArrayBounds,
        ),
        (
            "SELECT UBound(NULL) UNION ALL SELECT 1",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT UBound(1) UNION ALL SELECT 1",
            EvalError::TypeMismatch,
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(QueryError::Evaluation(error)),
            "{sql}"
        );
    }
}
