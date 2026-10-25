//! Fixed-point directories preserve payloads and admit bounded larger key sets.
use tablecraft_core::values::{Currency, Decimal, Value};
use tablecraft_query::{
    ExecutionOptions, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

const UNIQUE_ROWS: i32 = 12_000;

fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}

fn source(kind: ScalarType, values: Vec<Value>) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Records".into(),
            vec![
                Column::new("Unused".into(), ScalarType::Text, false).unwrap(),
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("K".into(), kind, true).unwrap(),
            ],
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    vec![
                        Value::Text("pruned".into()),
                        Value::Int32(i32::try_from(index).unwrap()),
                        value,
                    ]
                })
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn options(declared_integral: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        declared_integral,
        limits: QueryLimits {
            bytes: 128 * 1024 * 1024,
            rows: 300_000,
            steps: 5_000_000,
        },
        ..Default::default()
    }
}

fn execute(sql: &str, source: &MemorySource, declared: bool) -> Vec<Vec<Value>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    prepare(syntax.query(), source)
        .unwrap()
        .execute(source, &[], options(declared))
        .unwrap()
        .rows()
        .to_vec()
}

#[test]
fn twelve_thousand_currency_and_decimal_keys_fit_finite_work_in_both_profiles() {
    for kind in [ScalarType::Currency, ScalarType::Decimal] {
        let values: Vec<Value> = (0..UNIQUE_ROWS)
            .rev()
            .map(|number| match kind {
                ScalarType::Currency => {
                    Value::Currency(Currency::from_scaled(i64::from(number) * 10_000))
                }
                ScalarType::Decimal => decimal(i128::from(number) * 100, 2),
                _ => unreachable!(),
            })
            .collect();
        let source = source(kind, values.clone());
        for declared in [false, true] {
            assert_eq!(
                execute("SELECT DISTINCT K FROM Records", &source, declared),
                values
                    .iter()
                    .cloned()
                    .map(|value| vec![value])
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                execute(
                    "SELECT K,Count(*) FROM Records GROUP BY K",
                    &source,
                    declared
                ),
                values
                    .iter()
                    .cloned()
                    .map(|value| vec![value, Value::Int64(1)])
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn decimal_groups_retain_first_scale_and_null_keys_across_buffered_and_progressive_having() {
    let source = source(
        ScalarType::Decimal,
        vec![
            decimal(100, 2),
            decimal(10, 1),
            Value::Null,
            decimal(0, 28),
            decimal(0, 0),
            Value::Null,
        ],
    );
    let expected = vec![
        vec![decimal(100, 2), Value::Int64(2)],
        vec![Value::Null, Value::Int64(2)],
        vec![decimal(0, 28), Value::Int64(2)],
    ];
    let sql = "SELECT K,Count(*) FROM Records GROUP BY K HAVING Count(*)>1";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    for declared in [false, true] {
        assert_eq!(execute(sql, &source, declared), expected);
        let mut output = Vec::new();
        prepared
            .execute_progressive(&source, &[], options(declared), &mut |row| {
                output.push(row.to_vec());
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(output, expected);
    }
}

#[test]
fn fixed_union_distinct_prefix_does_not_remove_later_all_duplicates() {
    let source = source(
        ScalarType::Decimal,
        vec![decimal(10, 1), decimal(100, 2), decimal(20, 1)],
    );
    let sql = "SELECT K FROM Records WHERE Id=0
               UNION SELECT K FROM Records WHERE Id>0
               UNION ALL SELECT K FROM Records WHERE Id=1";
    for declared in [false, true] {
        assert_eq!(
            execute(sql, &source, declared),
            [
                vec![decimal(10, 1)],
                vec![decimal(20, 1)],
                vec![decimal(100, 2)]
            ]
        );
    }
}
