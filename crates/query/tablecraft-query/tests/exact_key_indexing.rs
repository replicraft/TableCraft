//! Common exact-key queries should fit work limits without quadratic comparisons.
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

const DISTINCT_ROWS: i32 = 12_000;
const INDEXED_WORK_ALLOWANCE: u64 = 5_000_000;

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Records".into(),
            vec![
                Column::new("Unused".into(), ScalarType::Text, false).unwrap(),
                Column::new("N".into(), ScalarType::Int32, false).unwrap(),
            ],
            (0..DISTINCT_ROWS)
                .map(|number| vec![Value::Text("pruned".into()), Value::Int32(number)])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn execute(sql: &str) -> Vec<Vec<Value>> {
    let source = source();
    let syntax = parse_query(sql, Default::default()).unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let result = prepared
        .execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 128 * 1024 * 1024,
                    rows: 300_000,
                    steps: INDEXED_WORK_ALLOWANCE,
                },
                ..Default::default()
            },
        )
        .unwrap();
    result.rows().to_vec()
}

#[test]
fn twelve_thousand_integral_groups_preserve_first_encounter_and_counts() {
    let rows = execute("SELECT N,Count(*) FROM Records GROUP BY N");
    assert_eq!(
        rows,
        (0..DISTINCT_ROWS)
            .map(|number| vec![Value::Int32(number), Value::Int64(1)])
            .collect::<Vec<_>>()
    );
}

#[test]
fn twelve_thousand_distinct_integral_rows_preserve_their_carriers() {
    let rows = execute("SELECT DISTINCT N FROM Records");
    assert_eq!(
        rows,
        (0..DISTINCT_ROWS)
            .map(|number| vec![Value::Int32(number)])
            .collect::<Vec<_>>()
    );
}

#[test]
fn union_rebuilds_each_distinct_prefix_and_retains_following_all_duplicates() {
    let rows = execute(
        "SELECT N FROM Records UNION SELECT N FROM Records
         UNION ALL SELECT N FROM Records WHERE N<3",
    );
    let mut expected = (0..DISTINCT_ROWS)
        .map(|number| vec![Value::Int32(number)])
        .collect::<Vec<_>>();
    expected.extend((0..3).map(|number| vec![Value::Int32(number)]));
    assert_eq!(rows, expected);
}
