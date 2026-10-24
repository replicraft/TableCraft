use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{OutputType, QueryError, Source, prepare, source::MemorySource};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::parse_query;

const FUNCTIONS: [&str; 12] = [
    "Abs", "Int", "Fix", "Round", "Sgn", "Sqr", "Exp", "Log", "Sin", "Cos", "Tan", "Atn",
];

struct MetadataOnly;

impl Source for MetadataOnly {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        panic!("closed math metadata must not request a native schema")
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("math metadata must not scan records")
    }
}

#[test]
fn null_math_composes_through_derived_subqueries_selectors_and_aggregates() {
    let runtime = MemorySource::default();
    for function in FUNCTIONS {
        for expression in [
            format!("{function}(NULL)"),
            format!("{function}(Abs(NULL))"),
            format!("(SELECT {function}(NULL))"),
            format!("IIf(True,{function}(NULL),NULL)"),
            format!("Sum({function}(NULL))"),
        ] {
            let sql = format!("SELECT D.N FROM (SELECT {expression} AS N) AS D");
            let query = parse_query(&sql, Default::default()).unwrap();
            let plan = prepare(query.query(), &MetadataOnly).unwrap();
            assert_eq!(plan.output_types().unwrap(), [OutputType::Null], "{sql}");

            let result = plan.execute(&runtime, &[], Default::default()).unwrap();
            assert_eq!(result.rows(), [vec![Value::Null]], "{sql}");
            assert_eq!(result.output_types(), [OutputType::Null]);
        }
    }
}

#[test]
fn null_math_union_arms_do_not_impose_a_numeric_conversion_on_other_arms() {
    let runtime = MemorySource::default();
    let expected = OutputType::Scalar {
        data_type: TypeDescriptor::new(ScalarType::Byte),
        nullable: true,
    };
    for function in FUNCTIONS {
        for sql in [
            format!("SELECT {function}(NULL) AS N UNION ALL SELECT CByte(2)"),
            format!("SELECT CByte(2) AS N UNION ALL SELECT {function}(NULL)"),
        ] {
            let query = parse_query(&sql, Default::default()).unwrap();
            let plan = prepare(query.query(), &MetadataOnly).unwrap();
            assert_eq!(plan.output_types().unwrap(), [expected], "{sql}");

            let result = plan.execute(&runtime, &[], Default::default()).unwrap();
            assert_eq!(result.rows().len(), 2, "{sql}");
            assert!(result.rows().contains(&vec![Value::Null]), "{sql}");
            assert!(result.rows().contains(&vec![Value::Byte(2)]), "{sql}");
            assert_eq!(result.output_types(), [expected]);
        }
    }
}

#[test]
fn null_metadata_keeps_round_controls_eager_failures_and_special_math_contracts() {
    let runtime = MemorySource::default();
    for (expression, expected) in [
        ("Round(NULL,29)", EvalError::Domain),
        ("Round(NULL,1/0)", EvalError::DivisionByZero),
        ("Round(NULL,CVErr(1))", EvalError::TypeMismatch),
        ("IIf(True,Abs(NULL),1/0)", EvalError::DivisionByZero),
        ("Round(NULL,(SELECT 1/0))", EvalError::DivisionByZero),
    ] {
        let query = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        let plan = prepare(query.query(), &MetadataOnly).unwrap();
        assert_eq!(
            plan.execute(&runtime, &[], Default::default()).err(),
            Some(QueryError::Evaluation(expected)),
            "{expression}"
        );
    }

    for (expression, kind) in [
        ("Rnd(NULL)", ScalarType::Single),
        ("RGB(NULL,0,0)", ScalarType::Int32),
        ("QBColor(NULL)", ScalarType::Int32),
    ] {
        let query = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        let plan = prepare(query.query(), &MetadataOnly).unwrap();
        assert_eq!(
            plan.output_types().unwrap(),
            [OutputType::Scalar {
                data_type: TypeDescriptor::new(kind),
                nullable: false,
            }],
            "{expression}"
        );
        assert!(plan.execute(&runtime, &[], Default::default()).is_err());
    }
}
