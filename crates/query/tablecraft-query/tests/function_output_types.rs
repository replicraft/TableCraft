use tablecraft_core::values::{Currency, Decimal, Double, ExtendedDateTime, Single, Value};
use tablecraft_expressions::functions::text::casing::CaseLocale;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{ParseLimits, parse_query};

const CASES: &[(&str, ScalarType)] = &[
    ("Len('abc')", ScalarType::Int32),
    ("LenB('abc')", ScalarType::Int32),
    ("InStr(1,'abc','b',0)", ScalarType::Int32),
    ("InStrRev('abc','b',-1,0)", ScalarType::Int32),
    ("StrComp('a','a',0)", ScalarType::Int16),
    ("Asc('a')", ScalarType::Int16),
    ("AscB('a')", ScalarType::Int16),
    ("AscW('a')", ScalarType::Int16),
    ("Val('2.5')", ScalarType::Double),
    ("LCase('A')", ScalarType::Text),
    ("UCase('a')", ScalarType::Text),
    ("LTrim(' a')", ScalarType::Text),
    ("RTrim('a ')", ScalarType::Text),
    ("Trim(' a ')", ScalarType::Text),
    ("Left('abc',1)", ScalarType::Text),
    ("Right('abc',1)", ScalarType::Text),
    ("Mid('abc',2,1)", ScalarType::Text),
    ("Replace('abc','b','x',1,-1,0)", ScalarType::Text),
    ("Space(2)", ScalarType::Text),
    ("String(2,'a')", ScalarType::Text),
    ("StrReverse('abc')", ScalarType::Text),
    ("ChrW(65)", ScalarType::Text),
    ("Hex(42)", ScalarType::Text),
    ("Oct(42)", ScalarType::Text),
    ("Str(42)", ScalarType::Text),
    ("CStr('abc')", ScalarType::Text),
    ("Partition(20,0,99,10)", ScalarType::Text),
    ("Year(#2026-10-22#)", ScalarType::Int16),
    ("Month(#2026-10-22#)", ScalarType::Int16),
    ("Day(#2026-10-22#)", ScalarType::Int16),
    ("Hour(#2026-10-22#)", ScalarType::Int16),
    ("Minute(#2026-10-22#)", ScalarType::Int16),
    ("Second(#2026-10-22#)", ScalarType::Int16),
    ("Weekday(#2026-10-22#)", ScalarType::Int16),
    ("DatePart('m',#2026-10-22#)", ScalarType::Int16),
    ("DateDiff('d',#2026-10-21#,#2026-10-22#)", ScalarType::Int32),
    ("DateAdd('d',1,#2026-10-21#)", ScalarType::DateTime),
    ("DateSerial(2026,10,22)", ScalarType::DateTime),
    ("TimeSerial(12,30,0)", ScalarType::DateTime),
    ("CDate(#2026-10-22#)", ScalarType::DateTime),
    ("DateValue(#2026-10-22#)", ScalarType::DateTime),
    ("TimeValue(#2026-10-22#)", ScalarType::DateTime),
    ("Sgn(-2)", ScalarType::Int16),
    ("Sqr(4)", ScalarType::Double),
    ("Exp(0)", ScalarType::Double),
    ("Log(1)", ScalarType::Double),
    ("Sin(0)", ScalarType::Double),
    ("Cos(0)", ScalarType::Double),
    ("Tan(0)", ScalarType::Double),
    ("Atn(0)", ScalarType::Double),
    ("RGB(1,2,3)", ScalarType::Int32),
    ("QBColor(1)", ScalarType::Int32),
    ("SLN(100,0,10)", ScalarType::Double),
    ("IsNumeric(NULL)", ScalarType::Boolean),
    ("IsDate(NULL)", ScalarType::Boolean),
    ("IsArray(NULL)", ScalarType::Boolean),
    ("IsEmpty(NULL)", ScalarType::Boolean),
    ("IsError(CVErr(5))", ScalarType::Boolean),
    ("TypeName(NULL)", ScalarType::Text),
    ("VarType(NULL)", ScalarType::Int16),
    (
        "GUIDFromString('{00000000-0000-0000-0000-000000000001}')",
        ScalarType::Binary,
    ),
    (
        "StringFromGUID(GUIDFromString('{00000000-0000-0000-0000-000000000001}'))",
        ScalarType::Text,
    ),
];

struct MetadataOnly(Vec<Column>);

impl Source for MetadataOnly {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.0)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("function metadata cannot inspect records")
    }
}

#[test]
fn successful_builtin_values_satisfy_their_metadata_without_binding_evaluation() {
    let source = MetadataOnly(vec![]);
    let runtime = MemorySource::new(vec![]).unwrap();
    let case_locale: CaseLocale = "und".parse().unwrap();
    for &(expression, expected) in CASES {
        let parsed = parse_query(&format!("SELECT {expression}"), ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let OutputType::Scalar {
            data_type,
            nullable,
        } = plan.output_types().unwrap()[0]
        else {
            panic!("missing metadata for {expression}");
        };
        assert_eq!(data_type.scalar_type(), expected, "{expression}");
        let result = plan
            .execute(
                &runtime,
                &[],
                ExecutionOptions {
                    case_locale: Some(&case_locale),
                    ..Default::default()
                },
            )
            .unwrap();
        let value = &result.rows()[0][0];
        assert!(nullable || !matches!(value, Value::Null), "{expression}");
        assert!(
            data_type.validate_value(value).is_ok(),
            "{expression}: {value:?}"
        );
        assert_eq!(result.output_types(), plan.output_types().unwrap());
    }
}

#[test]
fn clocks_random_and_provider_calls_have_metadata_without_activating_hosts() {
    let source = MetadataOnly(vec![
        Column::new("Unused".into(), ScalarType::Int16, false).unwrap(),
    ]);
    for (expression, expected) in [
        ("Now()", ScalarType::DateTime),
        ("Date()", ScalarType::DateTime),
        ("Time()", ScalarType::DateTime),
        ("Rnd()", ScalarType::Single),
        ("CurrentUser()", ScalarType::Text),
        ("MonthName(1)", ScalarType::Text),
        ("FormatCurrency(1)", ScalarType::Text),
        ("FormatDateTime(#2026-10-22#)", ScalarType::Text),
        ("Rate(12,-100,1000)", ScalarType::Double),
        ("NPV(.1,Array(1,2,3))", ScalarType::Double),
    ] {
        let sql = format!("SELECT {expression} FROM T");
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let OutputType::Scalar { data_type, .. } = plan.output_types().unwrap()[0] else {
            panic!("missing metadata for {expression}");
        };
        assert_eq!(data_type.scalar_type(), expected);
    }
}

#[test]
fn math_drops_operand_constraints_and_preserves_supported_carriers() {
    let source = MetadataOnly(vec![
        Column::with_type("D".into(), TypeDescriptor::decimal(8, 2).unwrap(), false).unwrap(),
    ]);
    for function in ["Abs", "Int", "Fix", "Round"] {
        let sql = format!("SELECT {function}(D),{function}(True),{function}(CByte(2)) FROM T");
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        for (output, expected) in plan.output_types().unwrap().iter().zip([
            ScalarType::Decimal,
            ScalarType::Int16,
            ScalarType::Byte,
        ]) {
            let OutputType::Scalar { data_type, .. } = output else {
                panic!("math metadata");
            };
            assert_eq!(*data_type, TypeDescriptor::new(expected));
        }
    }
    let parsed = parse_query("SELECT DateAdd('d',1,D) FROM T", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .output_types()
            .unwrap(),
        [OutputType::Dynamic]
    );
}

#[test]
fn derived_functions_preserve_types_for_empty_results() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("Label".into(), ScalarType::Text, true).unwrap()],
            vec![],
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query(
        "SELECT D.Name,Len(D.Name) FROM (SELECT UCase(Label) AS Name FROM T) AS D",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert!(result.rows().is_empty());
    for (output, expected) in result
        .output_types()
        .iter()
        .zip([ScalarType::Text, ScalarType::Int32])
    {
        let OutputType::Scalar { data_type, .. } = output else {
            panic!("derived metadata");
        };
        assert_eq!(data_type.scalar_type(), expected);
    }
}

#[test]
fn byte_intermediates_and_value_dependent_inputs_remain_dynamic() {
    let source = MetadataOnly(vec![]);
    let parsed = parse_query(
        "SELECT ChrB(65),LeftB('abc',1),RightB('abc',1),MidB('abc',1),StrConv('abc',64),Abs(IIf(True,CInt(1),CDbl(1)))",
        ParseLimits::default(),
    ).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .output_types()
            .unwrap(),
        [OutputType::Dynamic; 6]
    );
}

#[test]
fn carrier_preserving_math_matches_values_for_every_numeric_input_family() {
    let fields = [
        (ScalarType::Boolean, Value::Boolean(true)),
        (ScalarType::Byte, Value::Byte(2)),
        (ScalarType::Int16, Value::Int16(-2)),
        (ScalarType::Int32, Value::Int32(-2)),
        (ScalarType::Int64, Value::Int64(-2)),
        (ScalarType::Single, Value::Single(Single::from_float(-2.5))),
        (ScalarType::Double, Value::Double(Double::from_float(-2.5))),
        (
            ScalarType::Currency,
            Value::Currency(Currency::from_scaled(-25000)),
        ),
        (
            ScalarType::Decimal,
            Value::Decimal(Decimal::new(-250, 2).unwrap()),
        ),
    ];
    for (kind, value) in fields {
        let source = MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("N".into(), kind, true).unwrap()],
                vec![vec![value], vec![Value::Null]],
            )
            .unwrap(),
        ])
        .unwrap();
        let parsed = parse_query(
            "SELECT Abs(N),Int(N),Fix(N),Round(N) FROM T",
            ParseLimits::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let result = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        for row in result.rows() {
            for (output, value) in result.output_types().iter().zip(row) {
                let OutputType::Scalar {
                    data_type,
                    nullable,
                } = output
                else {
                    panic!("numeric math metadata");
                };
                assert!(*nullable || !matches!(value, Value::Null));
                assert!(data_type.validate_value(value).is_ok());
            }
        }
    }
}

#[test]
fn extended_date_add_keeps_its_carrier_through_a_derived_projection() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("Stamp".into(), ScalarType::DateTimeExtended, true).unwrap()],
            vec![
                vec![Value::DateTimeExtended(
                    ExtendedDateTime::new(2026, 10, 22, 12, 30, 0, 1234567).unwrap(),
                )],
                vec![Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query(
        "SELECT CVar(D.NextDay) FROM (SELECT DateAdd('d',1,Stamp) AS NextDay FROM T) AS D",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [OutputType::Scalar {
            data_type: TypeDescriptor::new(ScalarType::DateTimeExtended),
            nullable: true,
        }]
    );
    let result = plan
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::DateTimeExtended(
                ExtendedDateTime::new(2026, 10, 23, 12, 30, 0, 1234567).unwrap()
            )],
            vec![Value::Null],
        ]
    );
}
