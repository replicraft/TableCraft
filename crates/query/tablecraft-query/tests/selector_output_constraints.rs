use std::cell::Cell;
use tablecraft_core::values::{Decimal, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
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

fn source() -> Tracked {
    Tracked {
        data: MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![
                    Column::new("Flag".into(), ScalarType::Boolean, false).unwrap(),
                    Column::with_type("Short".into(), TypeDescriptor::text(2).unwrap(), true)
                        .unwrap(),
                    Column::with_type("Long".into(), TypeDescriptor::text(5).unwrap(), false)
                        .unwrap(),
                    Column::with_type("A".into(), TypeDescriptor::decimal(3, 1).unwrap(), false)
                        .unwrap(),
                    Column::with_type("B".into(), TypeDescriptor::decimal(5, 3).unwrap(), false)
                        .unwrap(),
                ],
                vec![
                    vec![
                        Value::Boolean(true),
                        Value::Text("é😀".into()),
                        Value::Text("abcde".into()),
                        Value::Decimal(Decimal::new(125, 1).unwrap()),
                        Value::Decimal(Decimal::new(12345, 3).unwrap()),
                    ],
                    vec![
                        Value::Boolean(false),
                        Value::Null,
                        Value::Text("abcde".into()),
                        Value::Decimal(Decimal::new(125, 1).unwrap()),
                        Value::Decimal(Decimal::new(12345, 3).unwrap()),
                    ],
                ],
            )
            .unwrap(),
        ])
        .unwrap(),
        scans: Cell::new(0),
    }
}

fn descriptor(data_type: TypeDescriptor, nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type,
        nullable,
    }
}

#[test]
fn same_carrier_selectors_relax_constraints_without_converting_selected_values() {
    for (expression, expected_type, expected_rows) in [
        (
            "IIf(Flag,[Short],[Long])",
            descriptor(TypeDescriptor::text(5).unwrap(), true),
            vec![Value::Text("é😀".into()), Value::Text("abcde".into())],
        ),
        (
            "Switch(Flag,[Short],True,[Long])",
            descriptor(TypeDescriptor::text(5).unwrap(), true),
            vec![Value::Text("é😀".into()), Value::Text("abcde".into())],
        ),
        (
            "Choose(IIf(Flag,1,2),[Short],[Long])",
            descriptor(TypeDescriptor::text(5).unwrap(), true),
            vec![Value::Text("é😀".into()), Value::Text("abcde".into())],
        ),
        (
            "Nz([Short],[Long])",
            descriptor(TypeDescriptor::text(5).unwrap(), false),
            vec![Value::Text("é😀".into()), Value::Text("abcde".into())],
        ),
        (
            "Nz([Short],'fallback')",
            descriptor(TypeDescriptor::new(ScalarType::Text), false),
            vec![Value::Text("é😀".into()), Value::Text("fallback".into())],
        ),
        (
            "IIf(Flag,A,B)",
            descriptor(TypeDescriptor::new(ScalarType::Decimal), false),
            vec![
                Value::Decimal(Decimal::new(125, 1).unwrap()),
                Value::Decimal(Decimal::new(12345, 3).unwrap()),
            ],
        ),
    ] {
        let source = source();
        let sql = format!("SELECT {expression} FROM T");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            plan.output_types().unwrap(),
            [expected_type],
            "{expression}"
        );
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert_eq!(
            result.rows(),
            expected_rows
                .into_iter()
                .map(|value| vec![value])
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn relaxed_text_metadata_composes_with_union_and_keeps_mixed_carriers_dynamic() {
    let source = source();
    let parsed = parse_query(
        "SELECT V FROM (SELECT IIf(Flag,[Short],[Long]) AS V FROM T UNION ALL SELECT 7) AS D",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.output_types().unwrap(),
        [descriptor(TypeDescriptor::new(ScalarType::Text), true)]
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
            vec![Value::Text("é😀".into())],
            vec![Value::Text("abcde".into())],
            vec![Value::Text("7".into())]
        ]
    );

    for expression in [
        "IIf(Flag,[Short],A)",
        "IIf(Flag,A,CSng(1))",
        "Nz([Short],A)",
    ] {
        let parsed =
            parse_query(&format!("SELECT {expression} FROM T"), Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .output_types()
                .unwrap(),
            [OutputType::Dynamic]
        );
    }
}

#[path = "selector_output_constraints/literals.rs"]
mod literals;
