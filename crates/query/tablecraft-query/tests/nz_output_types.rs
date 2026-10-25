use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

struct Views {
    rows: MemorySource,
    definition: Query,
    scans: Cell<usize>,
}

impl Views {
    fn new(empty: bool) -> Self {
        let rows = if empty {
            Vec::new()
        } else {
            vec![
                vec![Value::Int32(1), Value::Text("é".into()), Value::Null],
                vec![Value::Int32(2), Value::Null, Value::Int16(7)],
            ]
        };
        Self {
            rows: MemorySource::new(vec![
                MemoryTable::new(
                    "T".into(),
                    vec![
                        Column::new("ID".into(), ScalarType::Int32, false).unwrap(),
                        Column::with_type("Label".into(), TypeDescriptor::text(3).unwrap(), true)
                            .unwrap(),
                        Column::new("N".into(), ScalarType::Int16, true).unwrap(),
                    ],
                    rows,
                )
                .unwrap(),
            ])
            .unwrap(),
            definition: parse_query(
                "SELECT ID,Nz(Label) AS Label,Nz(N,CInt(0)) AS N FROM T",
                Default::default(),
            )
            .unwrap()
            .into_query(),
            scans: Cell::new(0),
        }
    }
}

impl Source for Views {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("V").then_some(&self.definition))
    }

    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.rows.columns(name)
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.rows.scan(name, visitor)
    }
}

fn short_text() -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::text(3).unwrap(),
        nullable: false,
    }
}

fn scalar(kind: ScalarType, nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(kind),
        nullable,
    }
}

#[test]
fn query_nz_default_is_text_and_explicit_nonnull_replacements_remove_nullability() {
    for empty in [false, true] {
        let source = Views::new(empty);
        let parsed = parse_query(
            "SELECT Nz(NULL),Nz(Label),Nz(N,CInt(0)),Nz(N,NULL),Nz(N),Nz(CVErr(5)) FROM T",
            Default::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            plan.output_types().unwrap(),
            [
                scalar(ScalarType::Text, false),
                short_text(),
                scalar(ScalarType::Int16, false),
                scalar(ScalarType::Int16, true),
                OutputType::Dynamic,
                OutputType::Dynamic,
            ]
        );
        // Empty Text fits the original maximum length; named/derived output
        // contracts must retain that bound without scanning any rows.
        let parsed =
            parse_query("SELECT ID,Label,N FROM V ORDER BY ID", Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            plan.output_types().unwrap(),
            [
                scalar(ScalarType::Int32, false),
                short_text(),
                scalar(ScalarType::Int16, false),
            ]
        );
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        if empty {
            assert!(result.rows().is_empty());
        } else {
            assert_eq!(
                result.rows(),
                [
                    vec![Value::Int32(1), Value::Text("é".into()), Value::Int16(0)],
                    vec![Value::Int32(2), Value::Text(String::new()), Value::Int16(7)],
                ]
            );
        }
    }
}

#[test]
fn nz_metadata_drives_union_text_conversion_without_evaluating_fallbacks_during_binding() {
    let source = Views::new(false);
    for sql in [
        "SELECT Nz(NULL) AS V UNION ALL SELECT 7",
        "SELECT V FROM (SELECT Nz(NULL) AS V UNION ALL SELECT 7) AS D",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            plan.output_types().unwrap(),
            [scalar(ScalarType::Text, false)]
        );
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(NumberFormat::Invariant),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [
                vec![Value::Text(String::new())],
                vec![Value::Text("7".into())]
            ]
        );
    }

    let parsed = parse_query("SELECT Nz(ID,CInt('bad')) FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.output_types().unwrap(),
        [scalar(ScalarType::Int32, false)]
    );
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
}

#[test]
fn checked_conversion_outputs_are_nonnull_without_suppressing_runtime_null_errors() {
    let source = Views::new(false);
    for (name, kind) in [
        ("CByte", ScalarType::Byte),
        ("CInt", ScalarType::Int16),
        ("CLng", ScalarType::Int32),
        ("CLngLng", ScalarType::Int64),
        ("CBool", ScalarType::Boolean),
        ("CSng", ScalarType::Single),
        ("CDbl", ScalarType::Double),
        ("CCur", ScalarType::Currency),
        ("CDec", ScalarType::Decimal),
    ] {
        let parsed =
            parse_query(&format!("SELECT {name}(NULL) FROM T"), Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(plan.output_types().unwrap(), [scalar(kind, false)]);
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(EvalError::InvalidNullConversion))
        );
        source.scans.set(0);
    }

    let parsed = parse_query("SELECT Nz(NULL)+1", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    // Mixed Text/numeric plus no longer promises one carrier for both profiles.
    assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic]);

    for declared_integral in [false, true] {
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral,
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
}
