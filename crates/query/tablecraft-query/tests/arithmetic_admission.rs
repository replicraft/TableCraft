use tablecraft_core::values::{ExtendedDateTime, Guid, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

const OPERATORS: [&str; 7] = ["+", "-", "*", "/", "\\", "Mod", "^"];

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}

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
        panic!("arithmetic metadata cannot inspect rows");
    }
}

#[test]
fn unsupported_native_scalar_families_do_not_disappear_after_a_buffered_null_prefix() {
    for (kind, value) in [
        (ScalarType::Binary, Value::Binary(vec![0, 1, 255])),
        (ScalarType::Guid, Value::Guid(Guid::from_bytes([1; 16]))),
        (
            ScalarType::DateTimeExtended,
            Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 1).unwrap()),
        ),
    ] {
        let columns = vec![Column::new("Raw".into(), kind, true).unwrap()];
        let metadata = MetadataOnly(columns.clone());
        let table = MemoryTable::new(
            "T".into(),
            columns.clone(),
            vec![vec![Value::Null], vec![value]],
        )
        .unwrap();
        let runtime = MemorySource::new(vec![table]).unwrap();

        for operator in OPERATORS {
            for expression in [
                format!("Null {operator} Raw"),
                format!("Raw {operator} Null"),
            ] {
                let query = syntax(&format!("SELECT {expression} FROM T"));
                let plan = prepare(&query, &metadata).unwrap();
                assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic]);
                for declared in [false, true] {
                    let options = ExecutionOptions {
                        declared_integral: declared,
                        ..Default::default()
                    };
                    assert_eq!(
                        plan.execute(&runtime, &[], options).err(),
                        Some(QueryError::Evaluation(EvalError::TypeMismatch)),
                        "{kind:?}: {expression}"
                    );
                }
            }
        }

        let filtered = syntax("SELECT Null+Raw FROM T WHERE Raw IS NULL");
        assert_eq!(
            prepare(&filtered, &metadata)
                .unwrap()
                .execute(&runtime, &[], Default::default())
                .unwrap()
                .rows(),
            [vec![Value::Null]]
        );

        let empty = MemorySource::new(vec![MemoryTable::new("T".into(), columns, vec![]).unwrap()])
            .unwrap();
        let query = syntax("SELECT Null+Raw FROM T");
        let result = prepare(&query, &metadata)
            .unwrap()
            .execute(&empty, &[], Default::default())
            .unwrap();
        assert!(result.rows().is_empty());
        assert_eq!(result.output_types(), [OutputType::Dynamic]);
    }
}

#[test]
fn projection_filters_aggregates_and_domains_retain_arithmetic_admission_failures() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("Raw".into(), ScalarType::Guid, true).unwrap()],
            vec![
                vec![Value::Null],
                vec![Value::Guid(Guid::from_bytes([1; 16]))],
            ],
        )
        .unwrap(),
    ])
    .unwrap();

    for sql in [
        "SELECT D.N FROM (SELECT Null+Raw AS N FROM T) AS D",
        "SELECT Count(*) FROM T WHERE Null+Raw IS NULL",
        "SELECT Sum(Null+Raw) FROM T",
        "SELECT DSum('Null+Raw','T')",
        "SELECT (SELECT Null+A.Raw) FROM T AS A",
    ] {
        let query = syntax(sql);
        assert_eq!(
            prepare(&query, &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(QueryError::Evaluation(EvalError::TypeMismatch)),
            "{sql}"
        );
    }
}

#[test]
fn null_metadata_does_not_fold_away_eager_errors_or_require_text_conversion_policies() {
    let source = MemorySource::default();
    for sql in [
        "SELECT Null+IIf(True,{guid {00112233-4455-6677-8899-aabbccddeeff}},1/0)",
        "SELECT (1/0)+(Null+GUIDFromString('{00112233-4455-6677-8899-aabbccddeeff}'))",
    ] {
        let query = syntax(sql);
        assert_eq!(
            prepare(&query, &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }

    for operator in OPERATORS {
        let query = syntax(&format!("SELECT Null {operator} 'bad numeric text'"));
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [OutputType::Null]);
        assert_eq!(
            plan.execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            [vec![Value::Null]]
        );
    }
}
