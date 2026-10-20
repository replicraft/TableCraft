use tablecraft_core::values::{Decimal, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{ParseLimits, parse_query};
fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![
                Column::new("Category".into(), ScalarType::Int32, false).unwrap(),
                Column::with_type(
                    "Amount".into(),
                    TypeDescriptor::decimal(28, 4).unwrap(),
                    true,
                )
                .unwrap(),
            ],
            vec![
                vec![Value::Int32(1), decimal(12500, 4)],
                vec![Value::Int32(1), decimal(25000, 4)],
                vec![Value::Int32(2), Value::Null],
                vec![Value::Int32(2), decimal(-5000, 4)],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source();
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}
#[test]
fn decimal_projection_filter_sort_and_normalized_float_mixing_share_the_kernel() {
    assert_eq!(
        run("SELECT Amount+2,Amount*0.1,Amount/2 FROM Items WHERE Amount>0 ORDER BY Amount DESC")
            .unwrap(),
        [
            vec![decimal(45000, 4), decimal(25000, 5), decimal(12500, 4)],
            vec![decimal(32500, 4), decimal(12500, 5), decimal(6250, 4)],
        ]
    );
    assert_eq!(
        run("SELECT CDec(2^95)+CDec(1),CDec(2)^CDec(3)").unwrap(),
        [vec![
            decimal(39614081257132168796771975169, 0),
            Value::Double(tablecraft_core::values::Double::from_float(8.0))
        ]]
    );
}
#[test]
fn decimal_sum_average_min_max_and_empty_groups_keep_null_and_scale_rules() {
    assert_eq!(run("SELECT Category,Sum(Amount),Avg(Amount),Min(Amount),Max(Amount),Count(Amount) FROM Items GROUP BY Category ORDER BY Category").unwrap(),[
        vec![Value::Int32(1),decimal(37500,4),decimal(18750,4),decimal(12500,4),decimal(25000,4),Value::Int64(2)],
        vec![Value::Int32(2),decimal(-5000,4),decimal(-5000,4),decimal(-5000,4),decimal(-5000,4),Value::Int64(1)]
    ]);
    assert_eq!(
        run("SELECT Sum(Amount),Avg(Amount),Count(Amount) FROM Items WHERE Category=3").unwrap(),
        [vec![Value::Null, Value::Null, Value::Int64(0)]]
    );
    assert_eq!(
        run("SELECT Category FROM Items GROUP BY Category HAVING Sum(Amount)>0").unwrap(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(
        run("SELECT Sum(CDec(Category)) FROM Items").unwrap(),
        [vec![decimal(6, 0)]]
    );
    assert_eq!(
        run("SELECT Sum(CDec(Amount)) FROM Items"),
        Err(QueryError::Evaluation(EvalError::InvalidNullConversion))
    );
    assert_eq!(
        run("SELECT Sum(CDec(Amount)) FROM Items WHERE Amount IS NOT NULL").unwrap(),
        [vec![decimal(32500, 4)]]
    );

    // SQL duplicate removal compares numeric values, not result scale.
    let distinct = run("SELECT CDec(1)/2 AS N UNION SELECT CDec(0.5)").unwrap();
    assert_eq!(distinct.len(), 1);
    assert!(matches!(distinct[0][0], Value::Decimal(_)));
}
#[test]
fn correlated_decimal_aggregates_retain_source_fields_and_error_visibility() {
    assert_eq!(run("SELECT A.Amount+(SELECT Sum(B.Amount) FROM Items AS B WHERE B.Category=A.Category) FROM Items AS A WHERE A.Amount>0 ORDER BY A.Amount").unwrap(),[vec![decimal(50000,4)],vec![decimal(62500,4)]]);
    assert_eq!(
        run("SELECT IIf(True,1,CDec(1)/0) FROM Items"),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT Sum(CDec(2^95)) FROM Items"),
        Err(QueryError::Evaluation(EvalError::Overflow))
    );
}
