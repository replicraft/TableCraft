use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare, source::MemorySource,
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn options() -> ExecutionOptions<'static> {
    ExecutionOptions {
        number_format: Some(NumberFormat::Invariant),
        collation: Some(&Ordinal),
        ..Default::default()
    }
}

fn text_type() -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(ScalarType::Text),
        nullable: true,
    }
}

#[test]
fn successful_byte_slice_text_supplies_union_common_conversion() {
    let source = MemorySource::default();
    for expression in [
        "LeftB('12',4)",
        "RightB('x12',4)",
        "MidB('x12y',3,4)",
        "StrConv('12',0)",
    ] {
        for operation in ["UNION", "UNION ALL"] {
            for reverse in [false, true] {
                let expression = format!("SELECT {expression} AS V");
                let sql = if reverse {
                    format!("SELECT 12 AS V {operation} {expression}")
                } else {
                    format!("{expression} {operation} SELECT 12")
                };
                let parsed = parse_query(&sql, Default::default()).unwrap();
                let plan = prepare(parsed.query(), &source).unwrap();
                assert_eq!(plan.output_types().unwrap(), [text_type()]);
                let result = plan.execute(&source, &[], options()).unwrap();
                let count = if operation == "UNION" { 1 } else { 2 };
                assert_eq!(
                    result.rows(),
                    vec![vec![Value::Text("12".into())]; count],
                    "{sql}"
                );
            }
        }
    }
}

#[test]
fn raw_byte_results_survive_union_all_observers_and_keep_strict_publication() {
    let source = MemorySource::default();
    for (expression, bytes) in [
        ("ChrB(65)", 1),
        ("LeftB('A',1)", 1),
        ("RightB('A',1)", 1),
        ("MidB('A',1,1)", 1),
        ("LeftB('😀',2)", 2),
        ("StrConv('A',128,1033)", 1),
    ] {
        let sql = format!(
            "SELECT LenB(Q.S),IsNull(Q.S) FROM (SELECT {expression} AS S UNION ALL SELECT 'A') AS Q"
        );
        let parsed = parse_query(&sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], options())
                .unwrap()
                .rows(),
            [
                vec![Value::Int32(bytes), Value::Boolean(false)],
                vec![Value::Int32(2), Value::Boolean(false)],
            ],
            "{sql}"
        );

        let sql = format!(
            "SELECT LenB(First(Q.S)) FROM (SELECT {expression} AS S UNION ALL SELECT 'A') AS Q"
        );
        let parsed = parse_query(&sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], options())
                .unwrap()
                .rows(),
            [vec![Value::Int32(bytes)]],
            "{sql}"
        );

        let sql = format!("SELECT {expression} UNION ALL SELECT 'A'");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [text_type()]);
        assert_eq!(
            plan.execute(&source, &[], options()).err(),
            Some(EvalError::UnrepresentableText.into()),
            "{sql}"
        );
    }

    // ANSI bytes 0x31,0x32 publish as UTF-16LE U+3231; metadata must not
    // reinterpret them as an already-decoded ANSI string or stored Binary.
    let parsed = parse_query(
        "SELECT StrConv('12',128,1033) UNION ALL SELECT 12",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options())
            .unwrap()
            .rows(),
        [
            vec![Value::Text("㈱".into())],
            vec![Value::Text("12".into())]
        ]
    );
}

struct Named {
    query: Query,
}

impl Source for Named {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("Bytes").then_some(&self.query))
    }

    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Err(QueryError::Source)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("source-free byte-result composition must not scan table rows")
    }
}

#[test]
fn named_derived_and_empty_metadata_remain_provider_free_and_do_not_decode_raw_strings() {
    for expression in [
        "LeftB('12',4)",
        "RightB('x12',4)",
        "MidB('x12',3,4)",
        "StrConv('12',0)",
    ] {
        let source = Named {
            query: parse_query(&format!("SELECT {expression} AS S"), Default::default())
                .unwrap()
                .into_query(),
        };
        for sql in [
            "SELECT S FROM Bytes UNION SELECT 12",
            "SELECT Q.S FROM (SELECT S FROM Bytes) AS Q UNION SELECT 12",
        ] {
            let parsed = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            assert_eq!(plan.output_types().unwrap(), [text_type()]);
            assert_eq!(
                plan.execute(&source, &[], options()).unwrap().rows(),
                [vec![Value::Text("12".into())]]
            );
        }
    }

    let source = MemorySource::default();
    for expression in ["ChrB(1/0)", "LeftB('A',1/0)", "StrConv('A',1)"] {
        let sql = format!("SELECT {expression} AS S WHERE False UNION ALL SELECT 12");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [text_type()]);
        assert_eq!(
            plan.execute(&source, &[], options()).unwrap().rows(),
            [vec![Value::Text("12".into())]]
        );
    }
    let source = Named {
        query: parse_query("SELECT ChrB(65) AS S", Default::default())
            .unwrap()
            .into_query(),
    };
    let parsed = parse_query("SELECT LenB(S) FROM Bytes", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Int32(1)]]
    );
}

#[test]
fn metadata_does_not_suppress_operand_formatting_or_encoding_failures() {
    let source = MemorySource::default();
    for (sql, error) in [
        (
            "SELECT LeftB('12',4) UNION ALL SELECT 1/0",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT LeftB('12',1/0) UNION ALL SELECT 12",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT StrConv('A',1) UNION ALL SELECT 12",
            EvalError::CaseLocaleRequired,
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], options())
                .err(),
            Some(error.into()),
            "{sql}"
        );
    }
    let parsed = parse_query(
        "SELECT LeftB('12',4) UNION ALL SELECT 12",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: None,
                    ..options()
                }
            )
            .err(),
        Some(EvalError::NumberFormatRequired.into())
    );
}
