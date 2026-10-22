use tablecraft_core::values::{Currency, Decimal, Double, Single, Value};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{ParseLimits, parse_query};

const FIELDS: [(&str, ScalarType); 10] = [
    ("Flag", ScalarType::Boolean),
    ("Octet", ScalarType::Byte),
    ("Short", ScalarType::Int16),
    ("Long", ScalarType::Int32),
    ("Big", ScalarType::Int64),
    ("Float", ScalarType::Single),
    ("Double", ScalarType::Double),
    ("Money", ScalarType::Currency),
    ("Exact", ScalarType::Decimal),
    ("Label", ScalarType::Text),
];
fn columns() -> Vec<Column> {
    FIELDS
        .into_iter()
        .map(|(name, kind)| Column::new(name.into(), kind, true).unwrap())
        .collect()
}
fn scalar(kind: ScalarType, nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(kind),
        nullable,
    }
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
        panic!("numeric result metadata cannot inspect records")
    }
}
fn metadata(sql: &str) -> Vec<OutputType> {
    let source = MetadataOnly(columns());
    prepare(
        parse_query(sql, ParseLimits::default()).unwrap().query(),
        &source,
    )
    .unwrap()
    .output_types()
    .unwrap()
    .to_vec()
}

#[test]
fn decimal_exponent_radix_and_stable_signed_literals_use_evaluator_carriers() {
    assert_eq!(
        metadata("SELECT .5,1e2,&HFFFF,&H10000,&O177777,&O200000,-1.25,+1e2,-1"),
        [
            scalar(ScalarType::Double, false),
            scalar(ScalarType::Double, false),
            scalar(ScalarType::Int16, false),
            scalar(ScalarType::Int32, false),
            scalar(ScalarType::Int16, false),
            scalar(ScalarType::Int32, false),
            scalar(ScalarType::Double, false),
            scalar(ScalarType::Double, false),
            scalar(ScalarType::Int64, false),
        ]
    );
    assert_eq!(
        metadata("SELECT -CInt(-32768), -CLng(-2147483648)"),
        [OutputType::Dynamic; 2]
    );
}

#[test]
fn accumulator_metadata_distinguishes_numerics_without_opening_domains() {
    assert_eq!(
        metadata(
            "SELECT Sum([Flag]),Sum([Octet]),Sum([Short]),Sum([Long]),Sum([Big]),Sum([Float]),Sum([Double]),Sum([Money]),Sum([Exact]),Sum([Label]) FROM T"
        ),
        [
            scalar(ScalarType::Int64, true),
            scalar(ScalarType::Int64, true),
            scalar(ScalarType::Int64, true),
            scalar(ScalarType::Int64, true),
            scalar(ScalarType::Int64, true),
            scalar(ScalarType::Double, true),
            scalar(ScalarType::Double, true),
            scalar(ScalarType::Currency, true),
            scalar(ScalarType::Decimal, true),
            OutputType::Dynamic,
        ]
    );
    assert_eq!(
        metadata(
            "SELECT Avg([Short]),Avg([Float]),Avg([Exact]),Avg([Money]),Var([Short]),VarP([Exact]),StDev([Money]),StDevP([Short]),DCount('*','T'),DVar('[Short]','T') FROM T"
        ),
        [
            scalar(ScalarType::Double, true),
            scalar(ScalarType::Double, true),
            scalar(ScalarType::Decimal, true),
            OutputType::Dynamic,
            scalar(ScalarType::Double, true),
            scalar(ScalarType::Double, true),
            scalar(ScalarType::Double, true),
            scalar(ScalarType::Double, true),
            scalar(ScalarType::Int64, true),
            scalar(ScalarType::Double, true),
        ]
    );
}
fn source(empty: bool, nulls: bool) -> MemorySource {
    let rows = if empty {
        vec![]
    } else if nulls {
        vec![vec![Value::Null; FIELDS.len()]]
    } else {
        [1, 3]
            .into_iter()
            .map(|n| {
                vec![
                    Value::Boolean(true),
                    Value::Byte(n as u8),
                    Value::Int16(n as i16),
                    Value::Int32(n),
                    Value::Int64(i64::from(n)),
                    Value::Single(Single::from_float(n as f32)),
                    Value::Double(Double::from_float(f64::from(n))),
                    Value::Currency(Currency::from_scaled(i64::from(n) * 10000)),
                    Value::Decimal(Decimal::new(i128::from(n) * 10, 1).unwrap()),
                    Value::Text(n.to_string()),
                ]
            })
            .collect()
    };
    MemorySource::new(vec![MemoryTable::new("T".into(), columns(), rows).unwrap()]).unwrap()
}

#[test]
fn metadata_matches_real_aggregates_empty_inputs_and_domain_null_semantics() {
    let sql = "SELECT Sum([Octet]),Sum([Float]),Sum([Money]),Sum([Exact]),Avg([Short]),Avg([Exact]),Var([Short]),StDevP([Money]),DCount('*','T','[AbsentColumn]=1') FROM T";
    for (empty, nulls) in [(false, false), (true, false), (false, true)] {
        let source = source(empty, nulls);
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let result = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(result.output_types(), plan.output_types().unwrap());
        assert_eq!(result.rows().len(), 1);
        assert_eq!(result.rows()[0].last(), Some(&Value::Null));
        for (kind, value) in result.output_types().iter().zip(&result.rows()[0]) {
            let OutputType::Scalar {
                data_type,
                nullable,
            } = kind
            else {
                panic!("known aggregate lost its carrier");
            };
            assert!(*nullable || !matches!(value, Value::Null));
            assert!(data_type.validate_value(value).is_ok());
        }
    }
}

#[test]
fn fixed_sum_crosstab_metadata_and_empty_cells_follow_the_accumulator() {
    let source = source(false, false);
    let parsed = parse_query(
        "TRANSFORM Sum([Short]) SELECT [Short] FROM T GROUP BY [Short] PIVOT Octet IN (1,2,3)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [
            scalar(ScalarType::Int16, true),
            scalar(ScalarType::Int64, true),
            scalar(ScalarType::Int64, true),
            scalar(ScalarType::Int64, true)
        ]
    );
    let result = plan
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.output_types(), plan.output_types().unwrap());
    assert!(result.rows().iter().all(|row| row[2] == Value::Null));
}
