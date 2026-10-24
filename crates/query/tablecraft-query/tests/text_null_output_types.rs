use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{OutputType, QueryError, Source, prepare};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

const CASES: &[&str] = &[
    "Len(NULL)",
    "LenB(NULL)",
    "LCase(NULL)",
    "UCase(NULL)",
    "LTrim(NULL)",
    "RTrim(NULL)",
    "Trim(NULL)",
    "Left(NULL,1)",
    "Right(NULL,1)",
    "Mid(NULL,1,1)",
    "LeftB(NULL,1)",
    "RightB(NULL,1)",
    "MidB(NULL,1,1)",
    "StrConv(NULL,1)",
    "Space(NULL)",
    "String(NULL,'a')",
    "String(1,NULL)",
    "Hex(NULL)",
    "Oct(NULL)",
    "Str(NULL)",
    "StrComp(NULL,'a',0)",
    "StrComp('a',NULL,0)",
    "InStr(NULL,'a')",
    "InStr('a',NULL)",
    "InStr(1,NULL,'a')",
    "InStr(1,'a',NULL)",
    "InStr(1,NULL,'a',0)",
    "InStr(1,'a',NULL,0)",
    "InStrB(NULL,'a')",
    "InStrB('a',NULL)",
    "InStrB(1,NULL,'a')",
    "InStrB(1,'a',NULL)",
    "InStrB(1,NULL,'a',0)",
    "InStrB(1,'a',NULL,0)",
    "InStrRev(NULL,'a',-1,0)",
    "InStrRev('a',NULL,-1,0)",
    "InStr(,NULL,'a')",
    "InStrB(1,'a',NULL,)",
    "StrComp(NULL,'a',)",
    "InStrRev('a',NULL,,)",
];

struct NamedNull(Query);

impl Source for NamedNull {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("NullText").then_some(&self.0))
    }

    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        panic!("closed text metadata must not request native columns")
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("text result metadata must not scan records")
    }
}

fn source(expression: &str) -> NamedNull {
    NamedNull(
        parse_query(&format!("SELECT {expression} AS N"), Default::default())
            .unwrap()
            .into_query(),
    )
}

#[test]
fn text_null_sources_survive_named_derived_and_scalar_composition_without_providers() {
    for expression in CASES {
        let source = source(expression);
        for sql in [
            "SELECT N FROM NullText",
            "SELECT Len(N) FROM NullText",
            "SELECT (SELECT N FROM NullText)",
            "SELECT D.N FROM (SELECT N FROM NullText) AS D",
        ] {
            let query = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(query.query(), &source).unwrap();
            assert_eq!(
                plan.output_types().unwrap(),
                [OutputType::Null],
                "{expression}: {sql}"
            );

            let result = plan.execute(&source, &[], Default::default()).unwrap();
            assert_eq!(result.rows(), [vec![Value::Null]], "{expression}: {sql}");
        }
    }
}

#[test]
fn null_text_results_do_not_impose_text_or_long_on_other_union_arms() {
    let expected = OutputType::Scalar {
        data_type: TypeDescriptor::new(ScalarType::Byte),
        nullable: true,
    };
    for expression in CASES {
        let source = source(expression);
        for sql in [
            "SELECT N FROM NullText UNION ALL SELECT CByte(2)",
            "SELECT CByte(2) AS N UNION ALL SELECT N FROM NullText",
        ] {
            let query = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(query.query(), &source).unwrap();
            assert_eq!(plan.output_types().unwrap(), [expected], "{expression}");

            let result = plan.execute(&source, &[], Default::default()).unwrap();
            assert_eq!(result.rows().len(), 2);
            assert!(
                result.rows().contains(&vec![Value::Byte(2)]),
                "{expression}"
            );
            assert!(result.rows().contains(&vec![Value::Null]), "{expression}");
        }
    }
}

#[test]
fn null_metadata_does_not_skip_controls_or_admit_null_refusing_functions() {
    for (expression, error) in [
        ("LeftB(NULL,-1)", EvalError::Domain),
        ("StrComp(NULL,'a',-2)", EvalError::Domain),
        ("Left(NULL,1/0)", EvalError::DivisionByZero),
        ("StrConv(NULL,1/0)", EvalError::DivisionByZero),
        ("String(NULL,1/0)", EvalError::DivisionByZero),
    ] {
        let source = source(expression);
        let query = parse_query("SELECT N FROM NullText", Default::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [OutputType::Null]);
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(error.into())
        );
    }

    for expression in ["CStr(NULL)", "Val(NULL)", "StrReverse(NULL)", "Asc(NULL)"] {
        let source = source(expression);
        let query = parse_query("SELECT N FROM NullText", Default::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_ne!(
            plan.output_types().unwrap(),
            [OutputType::Null],
            "{expression}"
        );
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(EvalError::InvalidNullConversion.into()),
            "{expression}"
        );
    }
}

#[test]
fn nullable_parameters_and_null_controls_do_not_become_null_only() {
    let source = source("NULL");
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT Len(P),Trim(P),MidB(P,1,2)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let metadata = plan.output_types().unwrap().to_vec();
    assert_eq!(
        metadata,
        [
            OutputType::Scalar {
                data_type: TypeDescriptor::new(ScalarType::Int32),
                nullable: true,
            },
            OutputType::Scalar {
                data_type: TypeDescriptor::new(ScalarType::Text),
                nullable: true,
            },
            OutputType::Dynamic,
        ]
    );
    assert_eq!(
        plan.execute(&source, &[("P", Value::Null)], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Null; 3]]
    );
    assert_eq!(
        plan.execute(
            &source,
            &[("P", Value::Text("x".into()))],
            Default::default()
        )
        .unwrap()
        .rows(),
        [vec![
            Value::Int32(1),
            Value::Text("x".into()),
            Value::Text("x".into())
        ]]
    );
    assert_eq!(plan.output_types().unwrap(), metadata);

    for expression in [
        "Left('a',NULL)",
        "InStr(NULL,'a','a',0)",
        "StrComp('a','a',NULL)",
        "StrConv('a',NULL)",
    ] {
        let source = NamedNull(
            parse_query(&format!("SELECT {expression} AS N"), Default::default())
                .unwrap()
                .into_query(),
        );
        let query = parse_query("SELECT N FROM NullText", Default::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_ne!(
            plan.output_types().unwrap(),
            [OutputType::Null],
            "{expression}"
        );
        assert!(
            plan.execute(&source, &[], Default::default()).is_err(),
            "{expression}"
        );
    }
}
