//! Successful ampersand carriers do not depend on an operand's Dynamic metadata.
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, OutputType, PreparationOptions, QueryError, prepare_with_options,
    source::{MemorySource, MemoryTable, Source},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

const KEY: &str = "IIf(Id=3,B,Id)&IIf(True,0,A)";

struct Data {
    data: MemorySource,
    view: Query,
    scans: Cell<usize>,
}

impl Data {
    fn new() -> Self {
        let table = MemoryTable::new(
            "T".into(),
            vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("N".into(), ScalarType::Int32, true).unwrap(),
                Column::new("A".into(), ScalarType::Text, true).unwrap(),
                Column::new("B".into(), ScalarType::Text, true).unwrap(),
            ],
            vec![
                vec![
                    Value::Int32(1),
                    Value::Null,
                    Value::Text("a".into()),
                    Value::Text("15".into()),
                ],
                vec![
                    Value::Int32(2),
                    Value::Int32(2),
                    Value::Null,
                    Value::Text("15".into()),
                ],
                vec![
                    Value::Int32(3),
                    Value::Int32(3),
                    Value::Text("b".into()),
                    Value::Text("15".into()),
                ],
            ],
        )
        .unwrap();
        Self {
            data: MemorySource::new(vec![table]).unwrap(),
            view: parse_query(&format!("SELECT {KEY} AS K FROM T"), Default::default())
                .unwrap()
                .into_query(),
            scans: Cell::new(0),
        }
    }
}

impl Source for Data {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.data.columns(name)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "V").then_some(&self.view))
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

fn text(nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(ScalarType::Text),
        nullable,
    }
}

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

static ORDINAL: Ordinal = Ordinal;

fn options(declared: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        declared_integral: declared,
        number_format: Some(NumberFormat::Invariant),
        collation: Some(&ORDINAL),
        ..Default::default()
    }
}

#[test]
fn dynamic_operands_still_prove_nonnull_text_beside_a_nonnull_convertible_operand() {
    let mut observed = Vec::new();
    for prepared_declared in [false, true] {
        let source = Data::new();
        let sql = format!("SELECT {KEY} AS K,VarType({KEY}) AS VT FROM T");
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare_with_options(
            syntax.query(),
            &source,
            PreparationOptions {
                declared_arithmetic: prepared_declared,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            source.scans.get(),
            0,
            "preparation must not infer from rows"
        );
        let public_type = plan.output_types().unwrap()[0];
        for declared in [false, true] {
            let result = plan.execute(&source, &[], options(declared)).unwrap();
            let expected = [
                vec![Value::Text("10".into()), Value::Int16(8)],
                vec![Value::Text("20".into()), Value::Int16(8)],
                vec![Value::Text("150".into()), Value::Int16(8)],
            ];
            assert_eq!(result.rows(), expected);
            observed.push((
                prepared_declared,
                declared,
                public_type,
                result.output_types()[0],
            ));
        }
    }
    let expected: Vec<_> = [false, true]
        .into_iter()
        .flat_map(|prepared| {
            [false, true]
                .into_iter()
                .map(move |declared| (prepared, declared, text(false), text(false)))
        })
        .collect();
    assert_eq!(
        observed, expected,
        "buffered runtime rows above prove successful Text carriers"
    );
}

#[test]
fn two_dynamic_operands_and_dynamic_null_are_nullable_text_without_losing_null_only() {
    let source = Data::new();
    let left = "IIf(Id=1,N,A)";
    let sql = format!("SELECT {left}&IIf(Id=2,N,B),{left}&NULL,NULL&NULL FROM T");
    let syntax = parse_query(&sql, Default::default()).unwrap();
    for prepared_declared in [false, true] {
        let plan = prepare_with_options(
            syntax.query(),
            &source,
            PreparationOptions {
                declared_arithmetic: prepared_declared,
                ..Default::default()
            },
        )
        .unwrap();
        for declared in [false, true] {
            let result = plan.execute(&source, &[], options(declared)).unwrap();
            let expected = [
                vec![Value::Text("15".into()), Value::Null, Value::Null],
                vec![Value::Text("2".into()), Value::Null, Value::Null],
                vec![
                    Value::Text("b15".into()),
                    Value::Text("b".into()),
                    Value::Null,
                ],
            ];
            assert_eq!(result.rows(), expected);
        }
        assert_eq!(
            plan.output_types().unwrap(),
            [text(true), text(true), OutputType::Null]
        );
    }
}

#[test]
fn named_derived_and_union_consumers_receive_the_proven_text_descriptor() {
    let source = Data::new();
    let syntax = parse_query(
        "SELECT K FROM V WHERE K='150' UNION ALL SELECT CLng(7) FROM T WHERE Id=1",
        Default::default(),
    )
    .unwrap();
    for prepared_declared in [false, true] {
        let plan = prepare_with_options(
            syntax.query(),
            &source,
            PreparationOptions {
                declared_arithmetic: prepared_declared,
                ..Default::default()
            },
        )
        .unwrap();
        for declared in [false, true] {
            let result = plan.execute(&source, &[], options(declared)).unwrap();
            assert_eq!(
                result.rows(),
                [
                    vec![Value::Text("150".into())],
                    vec![Value::Text("7".into())]
                ]
            );
            assert_eq!(result.output_types(), [text(false)]);
        }
    }
}

#[test]
fn new_success_descriptors_do_not_suppress_policies_errors_or_known_unsupported_families() {
    let source = Data::new();
    for sql in [
        "SELECT GUIDFromString('00112233-4455-6677-8899-aabbccddeeff')&'x' FROM T",
        "PARAMETERS G GUID; SELECT G&'x' FROM T",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
        assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic]);
    }
    for (sql, expected) in [
        (
            format!("SELECT {KEY} FROM T"),
            EvalError::NumberFormatRequired,
        ),
        (
            format!("SELECT {KEY},1/0 FROM T"),
            EvalError::DivisionByZero,
        ),
    ] {
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
        let execution = if expected == EvalError::NumberFormatRequired {
            ExecutionOptions::default()
        } else {
            options(false)
        };
        assert_eq!(
            plan.execute(&source, &[], execution).err().unwrap(),
            QueryError::Evaluation(expected)
        );
    }
}
