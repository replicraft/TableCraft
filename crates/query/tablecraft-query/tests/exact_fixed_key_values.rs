//! Exact signed rational oracles independent of key normalization and hashing.
use num_bigint::BigInt;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double, Value};
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, Source, prepare_with_options,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "exact_fixed_key_values/boundaries.rs"]
mod boundaries;
#[path = "exact_fixed_key_values/properties.rs"]
mod properties;
#[path = "exact_fixed_key_values/relational.rs"]
mod relational;

fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}

fn currency(scaled: i64) -> Value {
    Value::Currency(Currency::from_scaled(scaled))
}

fn rational_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Decimal(left), Value::Decimal(right)) => {
            // Arbitrary-precision cross products, without reducing either key.
            BigInt::from(left.coefficient()) * BigInt::from(10).pow(u32::from(right.scale()))
                == BigInt::from(right.coefficient()) * BigInt::from(10).pow(u32::from(left.scale()))
        }
        (Value::Currency(left), Value::Currency(right)) => {
            i128::from(left.scaled()) == i128::from(right.scaled())
        }
        (Value::Null, Value::Null) => true,
        (Value::Boolean(left), Value::Boolean(right)) => left == right,
        (Value::Int16(left), Value::Int16(right)) => left == right,
        (Value::Null, _) | (_, Value::Null) => false,
        _ => panic!("independent model expects a uniform fixed/integral tuple"),
    }
}

fn groups(rows: &[Vec<Value>]) -> Vec<(Vec<Value>, i64)> {
    let mut result: Vec<(Vec<Value>, i64)> = Vec::new();
    for row in rows {
        let existing = result.iter().position(|(key, _)| {
            key.len() == row.len()
                && key
                    .iter()
                    .zip(row)
                    .all(|(left, right)| rational_equal(left, right))
        });
        if let Some(index) = existing {
            result[index].1 += 1;
        } else {
            result.push((row.clone(), 1));
        }
    }
    result
}

fn source(columns: &[(&str, ScalarType)], rows: Vec<Vec<Value>>) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            columns
                .iter()
                .map(|(name, kind)| Column::new((*name).into(), *kind, true).unwrap())
                .collect(),
            rows,
        )
        .unwrap(),
    ])
    .unwrap()
}

fn buffered(
    sql: &str,
    source: &dyn Source,
    prepare_declared: bool,
    execute_declared: bool,
) -> Vec<Vec<Value>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare_with_options(
        syntax.query(),
        source,
        PreparationOptions {
            declared_arithmetic: prepare_declared,
            ..Default::default()
        },
    )
    .unwrap();
    plan.execute(
        source,
        &[],
        ExecutionOptions {
            declared_integral: execute_declared,
            ..Default::default()
        },
    )
    .unwrap()
    .rows()
    .to_vec()
}

fn assert_scalar_population(kind: ScalarType, keys: Vec<Value>) {
    let rows: Vec<_> = keys.into_iter().map(|key| vec![key]).collect();
    let expected = groups(&rows);
    let expected_distinct: Vec<_> = expected.iter().map(|(key, _)| key.clone()).collect();
    let expected_groups: Vec<_> = expected
        .iter()
        .map(|(key, count)| {
            let mut row = key.clone();
            row.push(Value::Int64(*count));
            row
        })
        .collect();
    let source = source(&[("K", kind)], rows);
    for declared in [false, true] {
        assert_eq!(
            buffered(
                "SELECT K,Count(*) FROM T GROUP BY K",
                &source,
                declared,
                declared
            ),
            expected_groups
        );
        assert_eq!(
            buffered("SELECT DISTINCT K FROM T", &source, declared, declared),
            expected_distinct
        );
    }
}
