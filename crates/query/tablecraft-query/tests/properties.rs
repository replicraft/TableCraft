use proptest::prelude::*;
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn source(values: &[Option<i64>]) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("V".into(), ScalarType::Int64, true).unwrap()],
            values
                .iter()
                .map(|v| vec![v.map_or(Value::Null, Value::Int64)])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}
fn execute(sql: &str, values: &[Option<i64>]) -> Vec<Value> {
    let source = source(values);
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap()
        .rows()
        .iter()
        .map(|r| r[0].clone())
        .collect()
}
proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x5441_424c_5155_4552),..Default::default()})]
    #[test]
    fn filter_sort_matches_an_independent_integer_model(values in prop::collection::vec(prop::option::of(-100_i64..100),0..40),floor in -100_i64..100) {
        let mut expected:Vec<_>=values.iter().filter_map(|n|*n).filter(|n|*n>floor).collect();expected.sort();
        let sql=format!("SELECT V FROM T WHERE V > {floor} ORDER BY V");
        prop_assert_eq!(execute(&sql,&values),expected.into_iter().map(Value::Int64).collect::<Vec<_>>());
    }
    #[test]
    fn distinct_preserves_one_null_and_sorted_unique_values(values in prop::collection::vec(prop::option::of(-10_i64..10),0..40)) {
        let mut expected=values.clone();expected.sort();expected.dedup();
        prop_assert_eq!(execute("SELECT DISTINCT V FROM T ORDER BY V",&values),expected.into_iter().map(|v|v.map_or(Value::Null,Value::Int64)).collect::<Vec<_>>());
    }
    #[test]
    fn count_and_sum_ignore_nulls(values in prop::collection::vec(prop::option::of(-100_i64..100),0..40)) {
        let nonnull:Vec<_>=values.iter().filter_map(|v|*v).collect();
        prop_assert_eq!(execute("SELECT Count(V) FROM T",&values),vec![Value::Int64(nonnull.len() as i64)]);
        let sum=if nonnull.is_empty(){Value::Null}else{Value::Int64(nonnull.iter().sum())};
        prop_assert_eq!(execute("SELECT Sum(V) FROM T",&values),vec![sum]);
    }
}
